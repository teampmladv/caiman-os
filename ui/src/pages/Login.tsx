import React, { useState, useRef, useEffect } from 'react'
import { motion } from 'framer-motion'
import { Loader2, Lock, User, AlertCircle } from 'lucide-react'
import axios from 'axios'

interface Props {
  onSuccess: (token: string) => void
}

const API_URL = import.meta.env.VITE_API_URL ?? 'http://localhost:8765'

export default function LoginPage({ onSuccess }: Props) {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [loading,  setLoading]  = useState(false)
  const [error,    setError]    = useState<string | null>(null)
  const userRef = useRef<HTMLInputElement>(null)

  useEffect(() => { userRef.current?.focus() }, [])

  const handleSubmit = async (e?: React.FormEvent | React.KeyboardEvent) => {
    e?.preventDefault()
    if (!username || !password || loading) return
    setLoading(true)
    setError(null)
    try {
      const r = await axios.post(
        `${API_URL}/auth/token`,
        { username, password },
        { headers: { 'Content-Type': 'application/json' } }
      )
      const token = r.data.token as string
      localStorage.setItem('caiman_token', token)
      localStorage.setItem('caiman_user',  r.data.username ?? username)
      localStorage.setItem('caiman_role',  r.data.role ?? 'admin')
      onSuccess(token)
    } catch (err: any) {
      const msg = err?.response?.status === 401
        ? 'Invalid credentials'
        : err?.response?.data?.error ?? err?.message ?? 'Authentication failed'
      setError(msg)
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen bg-caiman-bg flex items-center justify-center
                    relative overflow-hidden font-mono">

      {/* Animated background grid */}
      <div className="absolute inset-0 opacity-[0.04]"
           style={{
             backgroundImage: `
               linear-gradient(rgba(105,240,174,0.5) 1px, transparent 1px),
               linear-gradient(90deg, rgba(105,240,174,0.5) 1px, transparent 1px)`,
             backgroundSize: '40px 40px',
           }} />

      {/* Radial glow */}
      <div className="absolute inset-0 pointer-events-none"
           style={{
             background: 'radial-gradient(ellipse at center, rgba(27,94,32,0.15) 0%, transparent 60%)',
           }} />

      {/* Scanlines */}
      <div className="absolute inset-0 pointer-events-none opacity-30"
           style={{
             background: 'repeating-linear-gradient(0deg, transparent, transparent 2px, rgba(0,0,0,0.15) 2px, rgba(0,0,0,0.15) 4px)',
           }} />

      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.4, ease: 'easeOut' }}
        className="relative z-10 w-[400px] max-w-[90vw]"
      >
        {/* Logo / brand */}
        <div className="flex flex-col items-center mb-8">
          <motion.div
            initial={{ scale: 0 }}
            animate={{ scale: 1 }}
            transition={{ delay: 0.1, type: 'spring', stiffness: 200 }}
            className="w-14 h-14 rounded-full border-2 border-caiman-green
                       flex items-center justify-center mb-4 relative"
          >
            <div className="w-6 h-6 rounded-full bg-caiman-bright animate-heartbeat" />
            <div className="absolute inset-0 rounded-full border border-caiman-green/30 animate-ping" />
          </motion.div>
          <div className="font-display text-[28px] text-[#e8f5e9] tracking-wide leading-none">
            Caimán
          </div>
          <div className="text-[9px] text-caiman-dim tracking-[4px] mt-2">
            HAVANA · v0.1.0
          </div>
        </div>

        {/* Login card */}
        <div className="bg-caiman-bg2 border border-caiman-border rounded-lg
                        shadow-[0_20px_60px_rgba(0,0,0,0.5)]
                        overflow-hidden">

          {/* Header */}
          <div className="px-5 py-3 border-b border-caiman-border
                          flex items-center gap-2 bg-caiman-bg3">
            <div className="flex gap-1.5">
              <div className="w-2.5 h-2.5 rounded-full bg-caiman-red/60" />
              <div className="w-2.5 h-2.5 rounded-full bg-caiman-amber/60" />
              <div className="w-2.5 h-2.5 rounded-full bg-caiman-green/60" />
            </div>
            <span className="text-[10px] text-caiman-dim tracking-[2px] ml-2 uppercase">
              Authentication required
            </span>
          </div>

          <form onSubmit={handleSubmit} className="px-6 py-6 flex flex-col gap-4">

            {/* Username */}
            <div>
              <label className="text-[8px] text-caiman-dim tracking-[2px] uppercase block mb-1.5">
                Username
              </label>
              <div className="relative">
                <User size={11} className="absolute left-3 top-1/2 -translate-y-1/2 text-caiman-dim" />
                <input
                  ref={userRef}
                  type="text"
                  value={username}
                  onChange={e => setUsername(e.target.value)}
                  autoComplete="username"
                  spellCheck={false}
                  className="w-full bg-caiman-bg border border-caiman-border rounded
                             pl-9 pr-3 py-2 text-[12px] text-[#e8f5e9]
                             focus:border-caiman-green focus:outline-none
                             transition-colors"
                  placeholder="admin"
                />
              </div>
            </div>

            {/* Password */}
            <div>
              <label className="text-[8px] text-caiman-dim tracking-[2px] uppercase block mb-1.5">
                Password
              </label>
              <div className="relative">
                <Lock size={11} className="absolute left-3 top-1/2 -translate-y-1/2 text-caiman-dim" />
                <input
                  type="password"
                  value={password}
                  onChange={e => setPassword(e.target.value)}
                  autoComplete="current-password"
                  className="w-full bg-caiman-bg border border-caiman-border rounded
                             pl-9 pr-3 py-2 text-[12px] text-[#e8f5e9]
                             focus:border-caiman-green focus:outline-none
                             transition-colors"
                  placeholder="********"
                />
              </div>
            </div>

            {/* Error */}
            {error && (
              <motion.div
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: 'auto' }}
                className="flex items-center gap-2 text-[10px] text-caiman-red
                           bg-[#2e0a0a] border border-[#4a1a1a] rounded px-2.5 py-2"
              >
                <AlertCircle size={11} className="flex-shrink-0" />
                <span>{error}</span>
              </motion.div>
            )}

            {/* Submit */}
            <button
              type="submit"
              disabled={loading || !username || !password}
              className="mt-2 h-9 rounded bg-caiman-green/10 border border-caiman-green/40
                         text-caiman-bright text-[11px] tracking-[2px] uppercase
                         hover:bg-caiman-green/20 hover:border-caiman-green
                         disabled:opacity-30 disabled:cursor-not-allowed
                         transition-all duration-150
                         flex items-center justify-center gap-2"
            >
              {loading
                ? <><Loader2 size={12} className="animate-spin" /> Authenticating...</>
                : <>Sign in</>
              }
            </button>
          </form>

          {/* Footer */}
          <div className="px-5 py-2.5 border-t border-caiman-border bg-caiman-bg3
                          flex items-center justify-between text-[8px] text-caiman-dim tracking-[1px]">
            <span>JWT · 24h session</span>
            <span className="flex items-center gap-1.5">
              <div className="w-1 h-1 rounded-full bg-caiman-green animate-pulse" />
              api.caimanos.com
            </span>
          </div>
        </div>

        {/* Below card */}
        <div className="text-center mt-6 text-[8px] text-caiman-dim/60 tracking-[2px]">
          BORN IN CUBA · BUILT FOR THE CLOUD
        </div>
      </motion.div>
    </div>
  )
}
