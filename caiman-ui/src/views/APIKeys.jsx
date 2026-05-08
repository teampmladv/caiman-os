import { useState } from 'react'
import { useApp } from '../store.jsx'
import { C, Card, Badge, Btn, SectionHeader, Scroller } from '../components/UI.jsx'
import { getActiveCluster } from '../components/clusters/ClusterStore.js'

const ROLE_INFO = {
  'read-only': { label: 'Read-Only', color: '#00d4ff', desc: 'View VMs, metrics, logs' },
  operator:    { label: 'Operator',  color: '#ffb800', desc: '+ Start, stop, snapshot VMs' },
  admin:       { label: 'Admin',     color: '#ff4466', desc: '+ Create VMs, manage cluster, generate tokens' },
}

function TokenGenerator() {
  const cluster = getActiveCluster()
  const [form, setForm]     = useState({ name:'', role:'operator', expires:'30d' })
  const [result, setResult] = useState(null)
  const [loading, setLoading] = useState(false)
  const [error, setError]   = useState('')
  const [copied, setCopied] = useState(false)

  const generate = async () => {
    if (!form.name) { setError('Name is required'); return }
    if (!cluster)   { setError('No cluster connected -- use the cluster switcher in the topbar'); return }
    setError(''); setLoading(true)
    try {
      const res = await fetch(`${cluster.url}/auth/token`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${cluster.token}` },
        body: JSON.stringify(form),
      })
      const data = await res.json()
      if (!res.ok) throw new Error(data.error || 'Generation failed')
      setResult(data)
    } catch(e) { setError(e.message) }
    finally { setLoading(false) }
  }

  const copy = () => { navigator.clipboard.writeText(result.token); setCopied(true); setTimeout(() => setCopied(false), 2000) }

  return (
    <Card style={{ marginBottom:16 }}>
      <SectionHeader title="Generate Token" />
      {!cluster && (
        <div style={{ fontSize:11, color:'#ffb800', background:'#ffb80011', border:'1px solid #ffb80033', padding:'8px 12px', borderRadius:3, marginBottom:12 }}>
          Connect a cluster first using the switcher in the topbar
        </div>
      )}

      <div style={{ display:'flex', flexDirection:'column', gap:12 }}>
        <div>
          <div style={{ fontSize:10, color:C.dim, letterSpacing:'0.08em', textTransform:'uppercase', marginBottom:6 }}>Token Name</div>
          <input style={{ width:'100%', boxSizing:'border-box', background:'#080c12', border:`1px solid ${C.brd}`, color:C.txt, padding:'8px 10px', fontSize:12, outline:'none', fontFamily:'IBM Plex Mono, monospace' }}
            placeholder="my-automation-token" value={form.name} onChange={e => setForm({...form, name:e.target.value})} />
        </div>

        <div>
          <div style={{ fontSize:10, color:C.dim, letterSpacing:'0.08em', textTransform:'uppercase', marginBottom:6 }}>Role</div>
          <div style={{ display:'flex', flexDirection:'column', gap:4 }}>
            {Object.entries(ROLE_INFO).map(([key, r]) => (
              <button key={key} onClick={() => setForm({...form, role:key})}
                style={{ display:'flex', flexDirection:'column', gap:2, textAlign:'left', background: form.role===key ? r.color+'11' : 'transparent', border:`1px solid ${form.role===key ? r.color : C.brd}`, padding:'8px 12px', cursor:'pointer', transition:'all 0.15s' }}>
                <span style={{ fontSize:12, fontWeight:600, color: form.role===key ? r.color : C.txt, fontFamily:'IBM Plex Mono, monospace' }}>{r.label}</span>
                <span style={{ fontSize:10, color:C.dim }}>{r.desc}</span>
              </button>
            ))}
          </div>
        </div>

        <div>
          <div style={{ fontSize:10, color:C.dim, letterSpacing:'0.08em', textTransform:'uppercase', marginBottom:6 }}>Expiration</div>
          <div style={{ display:'flex', gap:6, flexWrap:'wrap' }}>
            {['1h','7d','30d','1y','never'].map(v => (
              <button key={v} onClick={() => setForm({...form, expires:v})}
                style={{ background: form.expires===v ? '#00d4ff22' : 'transparent', border:`1px solid ${form.expires===v ? '#00d4ff' : C.brd}`, color: form.expires===v ? '#00d4ff' : C.dim, padding:'5px 14px', cursor:'pointer', fontSize:11, fontFamily:'IBM Plex Mono, monospace' }}>
                {v}
              </button>
            ))}
          </div>
        </div>

        {error && <div style={{ background:'#ff446611', border:'1px solid #ff446633', padding:'8px 10px', fontSize:11, color:'#ff4466' }}>{error}</div>}

        <Btn primary onClick={generate} disabled={loading} style={{ alignSelf:'flex-end' }}>
          {loading ? 'Generating...' : '+ Generate Token'}
        </Btn>
      </div>

      {result && (
        <div style={{ background:'#22c55e08', border:'1px solid #22c55e33', borderRadius:4, padding:14, marginTop:14, display:'flex', flexDirection:'column', gap:10 }}>
          <div style={{ display:'flex', justifyContent:'space-between', alignItems:'center' }}>
            <span style={{ fontSize:12, color:'#22c55e', fontWeight:700 }}>Token generated</span>
            <span style={{ fontSize:10, color:C.dim }}>{ROLE_INFO[result.role]?.label} · {result.expires_at ? `expires ${new Date(result.expires_at).toLocaleDateString()}` : 'never expires'}</span>
          </div>
          <div style={{ display:'flex', gap:8, alignItems:'center', background:'#080c12', padding:'8px 10px' }}>
            <code style={{ flex:1, fontSize:10, color:'#00d4ff', wordBreak:'break-all', fontFamily:'IBM Plex Mono, monospace' }}>{result.token}</code>
            <button onClick={copy} style={{ background:'#1e2a3a', border:'none', color:C.txt, padding:'4px 10px', cursor:'pointer', fontSize:10, whiteSpace:'nowrap', fontFamily:'IBM Plex Mono, monospace' }}>
              {copied ? 'Copied' : 'Copy'}
            </button>
          </div>
          <div style={{ fontSize:10, color:'#ffb800' }}>Copy this token now -- it will not be shown again</div>
        </div>
      )}
    </Card>
  )
}

export default function APIKeys() {
  const { apiKeys, revokeKey } = useApp()
  return (
    <Scroller>
      <TokenGenerator />

      <SectionHeader title="Active Tokens" />
      <div style={{ display:'flex', flexDirection:'column', gap:4, marginBottom:16 }}>
        {apiKeys.map(k => (
          <Card key={k.id} style={{ opacity: k.active ? 1 : 0.5 }}>
            <div style={{ display:'flex', alignItems:'flex-start', gap:14 }}>
              <div style={{ flex:1 }}>
                <div style={{ display:'flex', alignItems:'center', gap:8, marginBottom:4 }}>
                  <span style={{ fontSize:12, color:C.txt, fontWeight:500 }}>{k.name}</span>
                  <Badge color={k.active ? C.g : C.dim}>{k.active ? 'ACTIVE' : 'REVOKED'}</Badge>
                </div>
                <div style={{ fontSize:11, color:C.g, marginBottom:4 }}>{k.prefix}{'*'.repeat(24)}</div>
                <div style={{ display:'flex', gap:4, flexWrap:'wrap', marginBottom:4 }}>
                  {k.perms.map(p => <Badge key={p} color={C.blu}>{p}</Badge>)}
                </div>
                <div style={{ fontSize:10, color:C.dim }}>Created {k.created} · Last used {k.last}</div>
              </div>
              {k.active && <Btn small danger onClick={() => revokeKey(k.id)}>Revoke</Btn>}
            </div>
          </Card>
        ))}
      </div>

      <SectionHeader title="API Usage (last 7 days)" />
      <Card>
        <div style={{ display:'grid', gridTemplateColumns:'repeat(4,1fr)', gap:12 }}>
          {[['Total Calls','47.2K'],['Auth Errors','3'],['P50 Latency','12ms'],['Rate Limited','0']].map(([l,v]) => (
            <div key={l} style={{ textAlign:'center' }}>
              <div style={{ fontSize:20, fontFamily:'Syne', fontWeight:800, color:C.g }}>{v}</div>
              <div style={{ fontSize:10, color:C.dim }}>{l}</div>
            </div>
          ))}
        </div>
      </Card>
    </Scroller>
  )
}
