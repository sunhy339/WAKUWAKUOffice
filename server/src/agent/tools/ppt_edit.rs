use async_trait::async_trait;
use serde_json::json;

use crate::agent::tool::{OfficeTool, ToolContext, ToolResult};
use crate::db::project_repo;
use crate::llm::LlmClient;
use crate::models::{Artifact, ChatMessage, PptProject, Slide};

pub struct PptEditTool;

fn find_ppt_artifact(ctx: &ToolContext, project_id: &str) -> Option<Artifact> {
    let target_artifact_id = ctx
        .target
        .as_ref()
        .and_then(|target| target.artifact_id.as_deref());

    if let Some(artifact_id) = target_artifact_id {
        return ctx
            .prior_artifacts
            .iter()
            .find(|artifact| {
                artifact.id == artifact_id
                    && artifact.kind == "ppt"
                    && artifact
                        .content
                        .get("project_id")
                        .and_then(|value| value.as_str())
                        == Some(project_id)
            })
            .cloned();
    }

    ctx.prior_artifacts
        .iter()
        .find(|artifact| {
            artifact.kind == "ppt"
                && artifact
                    .content
                    .get("project_id")
                    .and_then(|value| value.as_str())
                    == Some(project_id)
        })
        .cloned()
}

fn validate_edited_slide(original: &Slide, edited: &Slide) -> Result<(), String> {
    if edited.id != original.id {
        return Err("模型返回的幻灯片 ID 与目标页不一致，已保留原页面".to_string());
    }
    if edited.elements.is_empty() {
        return Err("PPT 单页修改结果没有可展示的页面元素，已保留原页面".to_string());
    }
    if edited.elements.len() > 120 {
        return Err("PPT 单页修改结果元素过多，已保留原页面".to_string());
    }

    for (index, element) in edited.elements.iter().enumerate() {
        if [element.x, element.y, element.w, element.h]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(format!(
                "PPT 单页修改结果第 {} 个元素坐标无效，已保留原页面",
                index + 1
            ));
        }
        if element.w <= 0.0 || element.h <= 0.0 {
            return Err(format!(
                "PPT 单页修改结果第 {} 个元素尺寸无效，已保留原页面",
                index + 1
            ));
        }
        if element
            .text
            .as_deref()
            .map(|text| text.chars().count() > 20_000)
            .unwrap_or(false)
        {
            return Err(format!(
                "PPT 单页修改结果第 {} 个元素文本过长，已保留原页面",
                index + 1
            ));
        }
    }

    Ok(())
}

fn target_slide_index(project: &PptProject, slide_id: &str) -> Option<usize> {
    project.slides.iter().position(|slide| slide.id == slide_id)
}

fn extract_slide_payload(value: serde_json::Value) -> serde_json::Value {
    value.get("slide").cloned().unwrap_or(value)
}

#[async_trait]
impl OfficeTool for PptEditTool {
    fn name(&self) -> &str {
        "ppt_edit"
    }

    fn description(&self) -> &str {
        "修改已有 PPT 的指定单页。仅用于用户已经选中某一页并明确提出修改要求的场景，不重新生成整套 PPT。"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "instruction": {
                    "type": "string",
                    "description": "对当前选中幻灯片的具体修改要求，例如精简文字、调整标题或补充要点"
                }
            },
            "required": ["instruction"]
        })
    }

    fn produces_artifact(&self) -> bool {
        true
    }

    async fn call(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let target = match ctx.target.as_ref() {
            Some(target) if target.operation == "edit_slide" => target,
            _ => return ToolResult::err("PPT 单页编辑缺少有效的目标上下文"),
        };

        if let Some(kind) = target.kind.as_deref() {
            if kind != "ppt" {
                return ToolResult::err("当前目标不是 PPT 产物");
            }
        }

        let project_id = match ctx.project_id.as_deref() {
            Some(project_id) if !project_id.trim().is_empty() => project_id,
            _ => return ToolResult::err("PPT 单页编辑缺少项目 ID"),
        };
        let slide_id = match target.slide_id.as_deref() {
            Some(slide_id) if !slide_id.trim().is_empty() => slide_id,
            _ => return ToolResult::err("PPT 单页编辑缺少幻灯片 ID"),
        };
        let instruction = input
            .get("instruction")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .trim();
        if instruction.is_empty() {
            return ToolResult::err("修改要求不能为空");
        }
        if instruction.chars().count() > 4_000 {
            return ToolResult::err("修改要求过长，请控制在 4000 字以内");
        }

        let existing_artifact = find_ppt_artifact(ctx, project_id);
        if target.artifact_id.is_some() && existing_artifact.is_none() {
            return ToolResult::err("当前 PPT 产物已不存在，请重新打开后再修改");
        }
        if let Some(expected_version) = target.expected_version {
            match existing_artifact.as_ref() {
                Some(artifact) if artifact.version == expected_version => {}
                Some(artifact) => {
                    return ToolResult::err(format!(
                        "PPT 产物已更新（当前版本 V{}），请刷新后再修改",
                        artifact.version
                    ));
                }
                None => return ToolResult::err("无法确认 PPT 当前版本，请重新打开后再修改"),
            }
        }

        let mut project = match project_repo::load_ppt_project(project_id) {
            Ok(Some(project)) => project,
            Ok(None) => return ToolResult::err("PPT 项目不存在"),
            Err(error) => return ToolResult::err(format!("读取 PPT 项目失败: {error}")),
        };
        if project.owner_id != ctx.user_id {
            return ToolResult::err("无权修改当前 PPT 项目");
        }

        let slide_index = match target_slide_index(&project, slide_id) {
            Some(index) => index,
            None => return ToolResult::err("指定的幻灯片不存在，可能已被删除或重新排序"),
        };
        if let Some(expected_index) = target.slide_index {
            if expected_index != slide_index {
                return ToolResult::err("幻灯片顺序已变化，请刷新后再修改");
            }
        }
        let original_slide = project.slides[slide_index].clone();

        ctx.send(
            "state_update",
            json!({
                "phase": "running",
                "step": "分析当前幻灯片",
                "detail": format!("正在准备第 {} 页的修改内容...", slide_index + 1),
                "slide_id": slide_id,
                "slide_index": slide_index,
                "at": chrono::Utc::now().to_rfc3339(),
            }),
        );

        let current_slide_json = match serde_json::to_string_pretty(&original_slide) {
            Ok(value) => value,
            Err(error) => return ToolResult::err(format!("读取当前幻灯片失败: {error}")),
        };
        let prompt = format!(
            r#"你是专业的演示文稿编辑。请只修改下面这一个幻灯片，不要修改其它页面，也不要返回 Markdown。

当前幻灯片 JSON：
{current_slide_json}

用户修改要求：
{instruction}

请只返回严格 JSON，格式如下：
{{"slide":{{...完整的幻灯片对象...}},"summary":"一句话说明本次修改"}}

编辑要求：
- 必须返回完整 slide 对象，字段至少包含 id、layout、background、elements。
- 必须保留 id 为 "{slide_id}"，不要改变幻灯片 ID。
- 只修改用户要求涉及的内容；没有要求改变的布局、背景和元素尽量保持不变。
- elements 中的 x、y、w、h 必须是数字，文本内容要适合在 16:9 幻灯片中展示。
- 不要添加外部图片 URL，不要编造不存在的数据。
- 如果要求不明确，做最小、保守且可逆的修改。"#
        );
        let client = LlmClient::for_user(&ctx.user_id, ctx.preferred_model.as_deref()).await;
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "你是 PPT 单页编辑器，只输出严格 JSON。".to_string(),
                tool_calls: None,
                tool_call_id: None,
            },
            ChatMessage {
                role: "user".to_string(),
                content: prompt,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        ctx.send(
            "state_update",
            json!({
                "phase": "running",
                "step": "生成页面修改方案",
                "detail": format!("正在根据要求更新第 {} 页...", slide_index + 1),
                "slide_id": slide_id,
                "slide_index": slide_index,
                "at": chrono::Utc::now().to_rfc3339(),
            }),
        );

        let response = match client.chat(&messages, None).await {
            Ok(response) => response,
            Err(error) => return ToolResult::err(format!("PPT 单页修改调用失败: {error}")),
        };
        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_deref())
            .unwrap_or_default();
        let payload = match LlmClient::extract_json(content) {
            Ok(payload) => payload,
            Err(error) => return ToolResult::err(format!("PPT 单页修改结果解析失败: {error}")),
        };
        let summary = payload
            .get("summary")
            .and_then(|value| value.as_str())
            .unwrap_or("已完成当前页面修改")
            .chars()
            .take(240)
            .collect::<String>();
        let mut edited_slide: Slide = match serde_json::from_value(extract_slide_payload(payload)) {
            Ok(slide) => slide,
            Err(error) => return ToolResult::err(format!("PPT 单页修改结果不完整: {error}")),
        };
        if let Err(error) = validate_edited_slide(&original_slide, &edited_slide) {
            return ToolResult::err(error);
        }

        project.slides[slide_index] = edited_slide.clone();
        let now = chrono::Utc::now().to_rfc3339();
        project.updated_at = now.clone();
        project
            .history
            .get_or_insert_with(Vec::new)
            .push(json!({
                "type": "edit",
                "title": format!("AI 修改第 {} 页：{}", slide_index + 1, edited_slide.title.clone().unwrap_or_else(|| format!("第 {} 页", slide_index + 1))),
                "detail": summary.clone(),
                "slide_index": slide_index,
                "slide_title": edited_slide.title.clone(),
                "created_at": now,
            }));

        if let Err(error) = project_repo::save_ppt_project(&project) {
            return ToolResult::err(format!("保存 PPT 修改失败: {error}"));
        }

        let version = existing_artifact
            .as_ref()
            .map(|artifact| artifact.version + 1)
            .unwrap_or(1);
        let artifact_id = existing_artifact
            .as_ref()
            .map(|artifact| artifact.id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let artifact_content = json!({
            "project_id": project.id.clone(),
            "title": project.title.clone(),
            "theme": project.theme.clone(),
            "slide_count": project.slides.len(),
            "total_slides": project.slides.len(),
            "slides": project.slides.clone(),
            "history": project.history.clone(),
            "edited_slide_id": slide_id,
            "operation": "edit_slide",
        });
        let artifact = Artifact {
            id: artifact_id,
            kind: "ppt".to_string(),
            tool_kind: "ppt".to_string(),
            title: project.title.clone(),
            status: "ready".to_string(),
            content: artifact_content.clone(),
            version,
            created_at: existing_artifact
                .as_ref()
                .map(|item| item.created_at.clone())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        ctx.send(
            "slide_update",
            json!({
                "project_id": project.id.clone(),
                "title": project.title.clone(),
                "theme": project.theme.clone(),
                "slides": project.slides.clone(),
                "slide_count": project.slides.len(),
                "total_slides": project.slides.len(),
                "history": project.history.clone(),
                "current_index": slide_index,
                "edited_slide_id": slide_id,
                "operation": "edit_slide",
            }),
        );
        ctx.send(
            "state_update",
            json!({
                "phase": "done",
                "step": "完成页面修改",
                "detail": format!("第 {} 页已更新", slide_index + 1),
                "slide_id": slide_id,
                "slide_index": slide_index,
                "at": chrono::Utc::now().to_rfc3339(),
            }),
        );

        ToolResult::ok(
            format!("已完成第 {} 页修改：{summary}", slide_index + 1),
            vec![],
        )
        .with_data(json!({
            "project_id": project.id.clone(),
            "slide_id": slide_id,
            "slide_index": slide_index,
            "summary": summary,
            "operation": "edit_slide",
        }))
        .with_artifact_updates(vec![artifact])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SlideElement;

    fn sample_slide() -> Slide {
        Slide {
            id: "slide-1".to_string(),
            layout: "content".to_string(),
            background: "FFFFFF".to_string(),
            elements: vec![SlideElement {
                element_type: "text".to_string(),
                x: 1.0,
                y: 1.0,
                w: 4.0,
                h: 1.0,
                text: Some("原始内容".to_string()),
                font_size: Some(20.0),
                color: Some("111827".to_string()),
                bold: Some(false),
                italic: None,
                align: Some("left".to_string()),
                valign: Some("top".to_string()),
                fill: None,
                shape: None,
                path: None,
                rows: None,
                cols: None,
                table_data: None,
                chart_type: None,
                chart_data: None,
            }],
            notes: None,
            title: Some("测试页".to_string()),
        }
    }

    #[test]
    fn edited_slide_must_keep_target_id() {
        let original = sample_slide();
        let mut edited = original.clone();
        edited.id = "another-slide".to_string();

        let result = validate_edited_slide(&original, &edited);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("ID"));
    }

    #[test]
    fn edited_slide_rejects_invalid_geometry() {
        let original = sample_slide();
        let mut edited = original.clone();
        edited.elements[0].w = 0.0;

        let result = validate_edited_slide(&original, &edited);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("尺寸无效"));
    }

    #[test]
    fn edited_slide_rejects_non_finite_coordinates() {
        let original = sample_slide();
        let mut edited = original.clone();
        edited.elements[0].x = f64::NAN;

        let result = validate_edited_slide(&original, &edited);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("坐标无效"));
    }
}
