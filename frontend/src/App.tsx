import { Routes, Route, Navigate } from 'react-router-dom'
import { lazy, Suspense, useEffect, useState } from 'react'
import { useLocation } from 'react-router-dom'
import { useAuthStore } from '@/stores/auth-store'
import { authApi } from '@/api'
import Login from '@/pages/Login'
import ResetPassword from '@/pages/ResetPassword'
import { AppLayout } from '@/components/layout/AppLayout'

const Studio = lazy(() => import('@/pages/Studio'))
const FilesPage = lazy(() => import('@/pages/files/FilesPage'))

function Loading() {
  return <div className="flex items-center justify-center h-screen text-slate-400">加载中…</div>
}

/**
 * 工作台和文件中心共用一个页面生命周期。
 *
 * Studio 内部持有 SSE 请求、AbortController 以及未完成会话的本地状态。
 * 如果文件中心切换成另一条路由并卸载 Studio，正在生成的任务会失去前端
 * 订阅，返回时还可能触发历史会话恢复，从而表现为任务被取消、消息消失。
 * 这里保留两页同时挂载，仅切换可见性，让文件中心不会影响工作台任务。
 */
function WorkspaceRoute() {
  const location = useLocation()
  const isFilesPage = location.pathname === '/files'

  if (location.pathname !== '/' && !isFilesPage) {
    return <Navigate to="/" replace />
  }

  return (
    <>
      <div className={isFilesPage ? 'hidden' : 'contents'}>
        <Suspense fallback={<Loading />}>
          <Studio />
        </Suspense>
      </div>
      <div className={isFilesPage ? 'contents' : 'hidden'}>
        <Suspense fallback={<Loading />}>
          <AppLayout>
            <FilesPage />
          </AppLayout>
        </Suspense>
      </div>
    </>
  )
}

function App() {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated)
  const login = useAuthStore((s) => s.login)
  const [authReady, setAuthReady] = useState(false)

  useEffect(() => {
    let active = true

    const bootstrapAuth = async () => {
      try {
        const { data } = await authApi.getMode()
        // 本地模式不依赖浏览器里长期保存的旧 JWT。服务重启、密钥变更或本地数据迁移后，
        // 旧令牌可能已经失效；每次应用启动都刷新一次内部会话，避免工作台进入半登录状态。
        if (data.mode === 'local') {
          const response = await authApi.localSession()
          login(response.data.access_token, response.data.user)
        }
      } catch {
        // 后端不可用时保留登录页，页面会展示提交失败原因。
      } finally {
        if (active) setAuthReady(true)
      }
    }

    bootstrapAuth()
    return () => {
      active = false
    }
  }, [login])

  if (!authReady) return <Loading />

  if (!isAuthenticated) {
    return (
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route path="/reset-password" element={<ResetPassword />} />
        <Route path="/*" element={<Navigate to="/login" />} />
      </Routes>
    )
  }

  return (
    <Suspense fallback={<Loading />}>
      <Routes>
        <Route path="/reset-password" element={<ResetPassword />} />
        <Route path="/login" element={<Navigate to="/" />} />
        <Route path="/studio" element={<Navigate to="/" replace />} />
        <Route path="/*" element={<WorkspaceRoute />} />
      </Routes>
    </Suspense>
  )
}

export default App
