import { useState, type FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuthStore } from '@/stores/auth-store'
import { authApi } from '@/api'
import { Loader2 } from 'lucide-react'

const LOGO_URL = '/logo.png'

type AuthMode = 'login' | 'register' | 'forgot'

export default function Login() {
  const navigate = useNavigate()
  const login = useAuthStore((s) => s.login)
  const [mode, setMode] = useState<AuthMode>('login')
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [agreed, setAgreed] = useState(false)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  const switchMode = (nextMode: AuthMode) => {
    setMode(nextMode)
    setError('')
    setPassword('')
    setConfirmPassword('')
  }

  const handleSubmit = async (e?: FormEvent) => {
    e?.preventDefault()
    setError('')

    if (mode === 'forgot') {
      if (!email.trim()) {
        setError('请输入注册邮箱')
        return
      }
      setLoading(true)
      try {
        const response = await authApi.requestPasswordReset(email.trim())
        if (response.data.reset_url) {
          window.location.assign(response.data.reset_url)
          return
        }
        setError(response.data.message)
      } catch (err: any) {
        setError(err.response?.data?.detail || '提交失败，请稍后重试')
      } finally {
        setLoading(false)
      }
      return
    }

    if (!username.trim() || !password) {
      setError('请填写账号和密码')
      return
    }

    if (mode === 'register' && password !== confirmPassword) {
      setError('两次输入的密码不一致')
      return
    }

    if (!agreed) {
      setError('请先同意用户协议')
      return
    }

    setLoading(true)

    try {
      const response = mode === 'login'
        ? await authApi.login(username.trim(), password)
        : await authApi.register(username.trim(), email.trim(), password)
      login(response.data.access_token, response.data.user)
      navigate('/')
    } catch (err: any) {
      setError(err.response?.data?.detail || (mode === 'login' ? '登录失败，请检查账号和密码' : '注册失败，请稍后重试'))
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen bg-white px-4 py-8 text-surface-900">
      <main className="mx-auto flex min-h-[calc(100vh-4rem)] w-full max-w-xl flex-col items-center justify-center text-center">
        <div className="mb-5 flex h-16 w-16 items-center justify-center overflow-hidden rounded-2xl bg-white shadow-sm ring-1 ring-black/[0.06]">
          <img src={LOGO_URL} alt="WaKuOffice logo" className="h-full w-full object-cover" />
        </div>

        <h1 className="text-4xl font-extrabold tracking-tight text-surface-900">WaKuOffice</h1>
        <p className="mt-4 text-lg font-semibold text-surface-700">学习AI办公、掌握AI部署、运用AI提效</p>

        <div className="mt-8 w-full max-w-sm rounded-3xl border border-surface-100 bg-white p-6 text-left shadow-[0_18px_60px_rgba(15,23,42,0.08)] sm:p-8">
          {mode !== 'forgot' && <div className="mb-6 flex rounded-xl bg-surface-50 p-1 text-sm font-semibold">
            <button
              type="button"
              onClick={() => switchMode('login')}
              className={`flex-1 rounded-lg py-2.5 transition ${mode === 'login' ? 'bg-white text-primary-600 shadow-sm' : 'text-surface-500 hover:text-surface-700'}`}
            >
              登录
            </button>
            <button
              type="button"
              onClick={() => switchMode('register')}
              className={`flex-1 rounded-lg py-2.5 transition ${mode === 'register' ? 'bg-white text-primary-600 shadow-sm' : 'text-surface-500 hover:text-surface-700'}`}
            >
              注册
            </button>
          </div>}

          {mode === 'forgot' && (
            <div className="mb-6">
              <h2 className="text-xl font-bold text-surface-900">找回密码</h2>
              <p className="mt-2 text-sm leading-6 text-surface-500">输入注册邮箱，我们会发送密码重置链接。</p>
            </div>
          )}

          <form onSubmit={handleSubmit} className="space-y-4">
            {mode !== 'forgot' && <input
              className="h-12 w-full rounded-xl border border-surface-200 bg-white px-4 text-base outline-none transition placeholder:text-surface-300 focus:border-primary-500 focus:ring-4 focus:ring-primary-100"
              placeholder="请输入账号"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              autoComplete="username"
            />}

            {(mode === 'register' || mode === 'forgot') && (
              <input
                className="h-12 w-full rounded-xl border border-surface-200 bg-white px-4 text-base outline-none transition placeholder:text-surface-300 focus:border-primary-500 focus:ring-4 focus:ring-primary-100"
                placeholder={mode === 'forgot' ? '请输入注册邮箱' : '邮箱（选填）'}
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                autoComplete="email"
              />
            )}

            {mode !== 'forgot' && <input
              className="h-12 w-full rounded-xl border border-surface-200 bg-white px-4 text-base outline-none transition placeholder:text-surface-300 focus:border-primary-500 focus:ring-4 focus:ring-primary-100"
              placeholder="请输入密码"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
            />}

            {mode === 'register' && (
              <input
                className="h-12 w-full rounded-xl border border-surface-200 bg-white px-4 text-base outline-none transition placeholder:text-surface-300 focus:border-primary-500 focus:ring-4 focus:ring-primary-100"
                placeholder="请再次输入密码"
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                autoComplete="new-password"
              />
            )}

            {error && (
              <div className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-500">
                {error}
              </div>
            )}

            {mode !== 'forgot' && <label className="flex cursor-pointer items-center gap-2 text-sm text-surface-600">
              <input
                type="checkbox"
                checked={agreed}
                onChange={(e) => setAgreed(e.target.checked)}
                className="h-4 w-4 rounded border-surface-300 text-primary-600 focus:ring-primary-500"
              />
              <span>同意用户协议</span>
            </label>}

            <button type="submit" disabled={loading} className="flex h-11 w-full items-center justify-center rounded-xl bg-primary-600 px-6 text-base font-semibold text-white shadow-sm transition hover:bg-primary-700 disabled:cursor-not-allowed disabled:opacity-60">
              {loading ? (
                <>
                  <Loader2 className="mr-2 h-5 w-5 animate-spin" />
                  {mode === 'login' ? '登录中...' : mode === 'register' ? '注册中...' : '发送中...'}
                </>
              ) : (
                mode === 'login' ? '登录' : mode === 'register' ? '创建账号' : '发送重置链接'
              )}
            </button>
          </form>

          {mode === 'login' && (
            <button type="button" onClick={() => switchMode('forgot')} className="mt-4 w-full text-center text-sm font-medium text-primary-600 hover:text-primary-700">
              忘记密码？
            </button>
          )}
          {mode === 'forgot' && (
            <button type="button" onClick={() => switchMode('login')} className="mt-4 w-full text-center text-sm font-medium text-surface-600 hover:text-primary-600">
              返回登录
            </button>
          )}
        </div>

        <button type="button" onClick={() => navigate('/')} className="mt-5 text-sm font-medium text-surface-600 hover:text-primary-600">
          回到首页
        </button>

        <p className="mt-10 text-sm font-medium leading-7 text-surface-600">
          说明：此平台主要以学习 OpenAI 为主，请合理、合法、合规的使用相关资料！
          <a href="https://bugstack.cn/" target="_blank" rel="noreferrer" className="ml-1 text-primary-600 underline underline-offset-2">
            查看用户协议
          </a>
        </p>
      </main>
    </div>
  )
}
