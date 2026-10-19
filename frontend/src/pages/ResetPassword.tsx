import { useMemo, useState, type FormEvent } from 'react'
import { Link, useNavigate, useSearchParams } from 'react-router-dom'
import { Loader2, LockKeyhole } from 'lucide-react'
import { authApi } from '@/api'

export default function ResetPassword() {
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const token = useMemo(() => searchParams.get('token') || '', [searchParams])
  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [success, setSuccess] = useState('')

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault()
    setError('')
    setSuccess('')

    if (!token) {
      setError('重置链接无效，请重新申请')
      return
    }
    if (password.length < 6) {
      setError('密码至少 6 个字符')
      return
    }
    if (password !== confirmPassword) {
      setError('两次输入的密码不一致')
      return
    }

    setLoading(true)
    try {
      const response = await authApi.confirmPasswordReset(token, password)
      setSuccess(response.data.message)
      window.setTimeout(() => navigate('/login'), 1200)
    } catch (err: any) {
      setError(err.response?.data?.detail || '重置失败，请重新申请重置链接')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen bg-white px-4 py-8 text-surface-900">
      <main className="mx-auto flex min-h-[calc(100vh-4rem)] w-full max-w-xl flex-col items-center justify-center text-center">
        <div className="mb-5 flex h-16 w-16 items-center justify-center rounded-2xl bg-primary-50 text-primary-600 shadow-sm ring-1 ring-primary-100">
          <LockKeyhole className="h-8 w-8" aria-hidden="true" />
        </div>
        <h1 className="text-4xl font-extrabold tracking-tight text-surface-900">重置密码</h1>
        <p className="mt-4 text-lg font-semibold text-surface-700">设置一个新的登录密码</p>

        <form onSubmit={handleSubmit} className="mt-8 w-full max-w-sm space-y-4 rounded-3xl border border-surface-100 bg-white p-6 text-left shadow-[0_18px_60px_rgba(15,23,42,0.08)] sm:p-8">
          <input
            className="h-12 w-full rounded-xl border border-surface-200 bg-white px-4 text-base outline-none transition placeholder:text-surface-300 focus:border-primary-500 focus:ring-4 focus:ring-primary-100"
            placeholder="请输入新密码"
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            autoComplete="new-password"
          />
          <input
            className="h-12 w-full rounded-xl border border-surface-200 bg-white px-4 text-base outline-none transition placeholder:text-surface-300 focus:border-primary-500 focus:ring-4 focus:ring-primary-100"
            placeholder="请再次输入新密码"
            type="password"
            value={confirmPassword}
            onChange={(event) => setConfirmPassword(event.target.value)}
            autoComplete="new-password"
          />

          {error && <div className="rounded-lg bg-red-50 px-3 py-2 text-sm text-red-500">{error}</div>}
          {success && <div className="rounded-lg bg-emerald-50 px-3 py-2 text-sm text-emerald-600">{success}</div>}

          <button type="submit" disabled={loading || Boolean(success)} className="flex h-11 w-full items-center justify-center rounded-xl bg-primary-600 px-6 text-base font-semibold text-white shadow-sm transition hover:bg-primary-700 disabled:cursor-not-allowed disabled:opacity-60">
            {loading ? <><Loader2 className="mr-2 h-5 w-5 animate-spin" />提交中...</> : '确认重置'}
          </button>
          <Link to="/login" className="block text-center text-sm font-medium text-surface-600 hover:text-primary-600">返回登录</Link>
        </form>
      </main>
    </div>
  )
}
