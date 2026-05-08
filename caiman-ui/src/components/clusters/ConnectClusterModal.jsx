import { useState } from 'react'
import { addCluster, probeCluster, decodeTokenInfo } from './ClusterStore.js'
import { C } from '../UI.jsx'

const COLORS = [
  { id: 'cyan',   hex: '#00d4ff' },
  { id: 'green',  hex: '#22c55e' },
  { id: 'amber',  hex: '#ffb800' },
  { id: 'rose',   hex: '#ff4466' },
  { id: 'violet', hex: '#a855f7' },
]
const ROLE_BADGE = {
  admin:      { label: 'Admin',     color: '#ff4466' },
  operator:   { label: 'Operator',  color: '#ffb800' },
  'read-only':{ label: 'Read-Only', color: '#00d4ff' },
}

export default function ConnectClusterModal({ onClose, onConnected }) {
  const [step, setStep]     = useState('form')
  const [url, setUrl]       = useState('')
  const [token, setToken]   = useState('')
  const [name, setName]     = useState('')
  const [color, setColor]   = useState('green')
  const [error, setError]   = useState('')
  const [tokenInfo, setTokenInfo] = useState(null)

  const handleToken = (val) => { setToken(val); setTokenInfo(decodeTokenInfo(val)); }

  const handleConnect = async () => {
    if (!url || !token) { setError('URL and token are required'); return; }
    setError(''); setStep('testing');
    try { await probeCluster({ url, token }); setStep('done'); }
    catch (e) { setError(`Connection failed: ${e.message}`); setStep('error'); }
  }

  const handleSave = () => {
    const cluster = addCluster({ name: name || url.replace(/https?:\/\//, '').split('/')[0], url, token, color });
    onConnected(cluster); onClose();
  }

  return (
    <div style={{ position:'fixed', inset:0, zIndex:1000, background:'rgba(0,0,0,0.8)', backdropFilter:'blur(4px)', display:'flex', alignItems:'center', justifyContent:'center' }}
      onClick={e => e.target === e.currentTarget && onClose()}>
      <div style={{ background:'#0d1117', border:`1px solid ${C.brd}`, borderRadius:10, width:460, maxWidth:'95vw', color:C.txt, fontFamily:'IBM Plex Mono, monospace', boxShadow:'0 24px 80px #000a' }}>

        {/* Header */}
        <div style={{ display:'flex', alignItems:'center', justifyContent:'space-between', padding:'18px 22px 14px', borderBottom:`1px solid ${C.brd}` }}>
          <div style={{ display:'flex', gap:10, alignItems:'center' }}>
            <span style={{ fontSize:26 }}>🐊</span>
            <div>
              <div style={{ fontSize:16, fontWeight:700, fontFamily:'Syne, sans-serif' }}>Connect Cluster</div>
              <div style={{ fontSize:11, color:C.dim }}>Add a remote Caiman OS node</div>
            </div>
          </div>
          <button style={{ background:'none', border:'none', color:C.dim, fontSize:16, cursor:'pointer' }} onClick={onClose}>x</button>
        </div>

        {/* Form */}
        <div style={{ padding:'18px 22px', display:'flex', flexDirection:'column', gap:12 }}>
          <div>
            <div style={{ fontSize:10, color:C.dim, letterSpacing:'0.08em', textTransform:'uppercase', marginBottom:6 }}>API URL</div>
            <input style={{ width:'100%', boxSizing:'border-box', background:'#080c12', border:`1px solid ${C.brd}`, borderRadius:4, color:C.txt, padding:'9px 11px', fontSize:13, outline:'none' }}
              placeholder="https://api.mi-cluster.com" value={url} onChange={e => setUrl(e.target.value)} />
          </div>
          <div>
            <div style={{ fontSize:10, color:C.dim, letterSpacing:'0.08em', textTransform:'uppercase', marginBottom:6 }}>Token</div>
            <input style={{ width:'100%', boxSizing:'border-box', background:'#080c12', border:`1px solid ${C.brd}`, borderRadius:4, color:'#00d4ff', padding:'9px 11px', fontSize:11, outline:'none' }}
              placeholder="caim_eyJhbGc..." value={token} onChange={e => handleToken(e.target.value)} />
            {tokenInfo && (
              <div style={{ display:'flex', gap:8, alignItems:'center', marginTop:6 }}>
                <span style={{ fontSize:10, padding:'2px 7px', borderRadius:3, background: ROLE_BADGE[tokenInfo.role]?.color + '22', color: ROLE_BADGE[tokenInfo.role]?.color || C.dim }}>
                  {ROLE_BADGE[tokenInfo.role]?.label || tokenInfo.role}
                </span>
                <span style={{ fontSize:10, color:C.dim }}>{tokenInfo.name}</span>
                {tokenInfo.isExpired && <span style={{ fontSize:10, color:'#ff4466', fontWeight:700 }}>EXPIRED</span>}
              </div>
            )}
          </div>
          <div>
            <div style={{ fontSize:10, color:C.dim, letterSpacing:'0.08em', textTransform:'uppercase', marginBottom:6 }}>Display Name</div>
            <input style={{ width:'100%', boxSizing:'border-box', background:'#080c12', border:`1px solid ${C.brd}`, borderRadius:4, color:C.txt, padding:'9px 11px', fontSize:13, outline:'none' }}
              placeholder="hetzner-prod" value={name} onChange={e => setName(e.target.value)} />
          </div>
          <div>
            <div style={{ fontSize:10, color:C.dim, letterSpacing:'0.08em', textTransform:'uppercase', marginBottom:8 }}>Color</div>
            <div style={{ display:'flex', gap:8 }}>
              {COLORS.map(c => (
                <button key={c.id} onClick={() => setColor(c.id)} style={{ width:20, height:20, borderRadius:'50%', background:c.hex, border:'none', cursor:'pointer', boxShadow: color === c.id ? `0 0 0 2px #0a0e1a, 0 0 0 4px ${c.hex}` : 'none', transform: color === c.id ? 'scale(1.2)' : 'scale(1)', transition:'all 0.15s' }} />
              ))}
            </div>
          </div>
          {error && <div style={{ background:'#ff446611', border:'1px solid #ff446633', borderRadius:4, padding:'9px 11px', fontSize:12, color:'#ff4466' }}>{error}</div>}
        </div>

        {/* Footer */}
        <div style={{ display:'flex', gap:8, justifyContent:'flex-end', padding:'14px 22px', borderTop:`1px solid ${C.brd}` }}>
          <button style={{ background:'none', border:`1px solid ${C.brd}`, borderRadius:4, color:C.dim, padding:'8px 16px', cursor:'pointer', fontSize:12 }} onClick={onClose}>Cancel</button>
          {step !== 'done'
            ? <button style={{ background:'#00d4ff', border:'none', borderRadius:4, color:'#000', padding:'8px 18px', cursor:'pointer', fontSize:12, fontWeight:700, opacity: step==='testing'?0.7:1 }} onClick={handleConnect} disabled={step==='testing'}>
                {step === 'testing' ? 'Testing...' : 'Test & Connect'}
              </button>
            : <button style={{ background:'#22c55e', border:'none', borderRadius:4, color:'#000', padding:'8px 18px', cursor:'pointer', fontSize:12, fontWeight:700 }} onClick={handleSave}>
                Save Cluster
              </button>
          }
        </div>
      </div>
    </div>
  )
}
