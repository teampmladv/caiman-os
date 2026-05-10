import { useState } from 'react'
import { addCluster, setActiveCluster } from '../components/clusters/ClusterStore.js'

const API_URL = 'https://api.caimanos.com'

export default function Login({ onLogin }) {
  const [username, setUsername] = useState('admin')
  const [password, setPassword] = useState('')
  const [error, setError]       = useState('')
  const [loading, setLoading]   = useState(false)

  const handleLogin = async (e) => {
    e.preventDefault()
    setLoading(true)
    setError('')
    try {
      const res = await fetch(`${API_URL}/auth/token`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password }),
      })
      const data = await res.json()
      if (!res.ok) throw new Error(data.error || 'Login failed')
      const cluster = addCluster({
        name:  'caiman-bare-01',
        url:   API_URL,
        token: data.token,
        color: '#22c55e',
      })
      setActiveCluster(cluster.id)
      onLogin()
    } catch (err) {
      setError(err.message)
    } finally {
      setLoading(false)
    }
  }

  return (
    <div style={{
      minHeight: '100vh', display: 'flex', alignItems: 'center',
      justifyContent: 'center', background: '#0a0f1a',
    }}>
      <div style={{
        background: '#111827', border: '1px solid #1f2937',
        borderRadius: 12, padding: '2.5rem', width: 360,
      }}>
        <div style={{ textAlign: 'center', marginBottom: '2rem' }}>
          <div style={{ fontSize: 40, marginBottom: 8 }}>🐊</div>
          <div style={{ fontFamily: 'Syne, sans-serif', fontSize: 22, fontWeight: 700, color: '#fff' }}>
            Caiman OS
          </div>
          <div style={{ color: '#6b7280', fontSize: 13, marginTop: 4 }}>
            Hypervisor Management Platform
          </div>
        </div>
        <form onSubmit={handleLogin}>
          <div style={{ marginBottom: 16 }}>
            <label style={{ color: '#9ca3af', fontSize: 12, display: 'block', marginBottom: 6 }}>USERNAME</label>
            <input value={username} onChange={e => setUsername(e.target.value)}
              style={{ width: '100%', background: '#1f2937', border: '1px solid #374151',
                borderRadius: 8, padding: '10px 12px', color: '#fff', fontSize: 14,
                outline: 'none', boxSizing: 'border-box' }} autoFocus />
          </div>
          <div style={{ marginBottom: 24 }}>
            <label style={{ color: '#9ca3af', fontSize: 12, display: 'block', marginBottom: 6 }}>PASSWORD</label>
            <input type="password" value={password} onChange={e => setPassword(e.target.value)}
              style={{ width: '100%', background: '#1f2937', border: '1px solid #374151',
                borderRadius: 8, padding: '10px 12px', color: '#fff', fontSize: 14,
                outline: 'none', boxSizing: 'border-box' }} />
          </div>
          {error && (
            <div style={{ background: '#1f0a0a', border: '1px solid #7f1d1d',
              borderRadius: 8, padding: '10px 12px', color: '#f87171',
              fontSize: 13, marginBottom: 16 }}>{error}</div>
          )}
          <button type="submit" disabled={loading}
            style={{ width: '100%', background: '#16a34a', color: '#fff',
              border: 'none', borderRadius: 8, padding: '11px',
              fontSize: 14, fontWeight: 600, cursor: loading ? 'wait' : 'pointer',
              opacity: loading ? 0.7 : 1 }}>
            {loading ? 'Authenticating...' : 'Sign In'}
          </button>
        </form>
        <div style={{ textAlign: 'center', marginTop: 20, color: '#374151', fontSize: 12 }}>
          Caiman OS v1.3.0 · Capablanca Digital
        </div>
      </div>
    </div>
  )
}
