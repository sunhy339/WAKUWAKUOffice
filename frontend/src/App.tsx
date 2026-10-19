import { Routes, Route, Navigate } from 'react-router-dom'
import { lazy, Suspense, useEffect, useState } from 'react'
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
        <Route path="/" element={<Studio />} />
        <Route element={<AppLayout />}>
          <Route path="/files" element={<FilesPage />} />
          <Route path="/studio" element={<Navigate to="/" replace />} />
          <Route path="/*" element={<Navigate to="/" />} />
        </Route>
      </Routes>
    </Suspense>
  )
}

export default App
