import { useState, useEffect } from 'react'
import { getActiveCluster } from '../components/clusters/ClusterStore.js'
import { C, Card, Bar, Badge, StatusDot, MetricCard, SectionHeader, Scroller } from '../components/UI.jsx'
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts'

const COLOR_HEX = { cyan:'#00d4ff', green:'#22c55e', amber:'#ffb800', rose:'#ff4466', violet:'#a855f7' }

async function apiFetch(cluster, path) {
  const res = await fetch(`${cluster.url}${path}`, {
    headers: { Authorization: `Bearer ${cluster.token}` },
    signal: AbortSignal.timeout(8000),
  })
  if (!res.ok) throw new Error(`HTTP ${res.status}`)
  return res.json()
}

export default function ClusterView() {
  const cluster = getActiveCluster()
  const [data, setData]       = useState(null)
  const [error, setError]     = useState(null)
  const [loading, setLoading] = useState(true)
  const [history, setHistory] = useState([])

  const load = async () => {
    const cluster = getActiveCluster()
    if (!cluster) return
    setLoading(true); setError(null)
    try {
      const [clusterData, vms] = await Promise.all([
        apiFetch(cluster, '/api/cluster'),
        apiFetch(cluster, '/api/vms'),
      ])
      setData({ ...clusterData, vms })
      setHistory(h => [...h.slice(-19), { t: h.length, cpu: clusterData.totalCpuPct || 0 }])
    } catch(e) { setError(e.message) }
    finally { setLoading(false) }
  }

  useEffect(() => {
    load()
    const interval = setInterval(load, 5000)
    const handler = () => load()
    window.addEventListener('caiman:cluster-changed', handler)
    return () => { clearInterval(interval); window.removeEventListener('caiman:cluster-changed', handler) }
  }, [])

  const hex = cluster ? (COLOR_HEX[cluster.color] || '#22c55e') : '#22c55e'

  if (!cluster) return (
    <div style={{ display:'flex', flexDirection:'column', alignItems:'center', justifyContent:'center', height:'100%', gap:12 }}>
      <div style={{ fontSize:32 }}>🐊</div>
      <div style={{ fontSize:14, color:C.dim }}>No cluster connected</div>
      <div style={{ fontSize:11, color:C.muted }}>Use the sidebar to connect a cluster</div>
    </div>
  )

  if (loading && !data) return (
    <div style={{ display:'flex', alignItems:'center', justifyContent:'center', height:'100%' }}>
      <div style={{ fontSize:11, color:C.dim, fontFamily:'IBM Plex Mono, monospace' }}>Connecting to {cluster.name}...</div>
    </div>
  )

  if (error) return (
    <div style={{ display:'flex', flexDirection:'column', alignItems:'center', justifyContent:'center', height:'100%', gap:12 }}>
      <div style={{ fontSize:11, color:C.red, fontFamily:'IBM Plex Mono, monospace' }}>{error}</div>
      <button onClick={load} style={{ background:'none', border:`1px solid ${C.brd}`, color:C.dim, padding:'6px 16px', cursor:'pointer', fontSize:11 }}>Retry</button>
    </div>
  )

  const nodes   = data?.nodes || []
  const vms     = data?.vms   || []
  const running = vms.filter(v => v.status === 'RUNNING').length
  const avgCpu  = nodes.length ? Math.round(nodes.reduce((a,n) => a + (n.cpu_usage_pct||0), 0) / nodes.length) : 0
  const avgMem  = nodes.length ? Math.round(nodes.reduce((a,n) => a + (n.mem_usage_pct||0), 0) / nodes.length) : 0

  return (
    <Scroller>
      {/* Cluster header */}
      <div style={{ display:'flex', alignItems:'center', gap:10, marginBottom:16 }}>
        <span style={{ width:10, height:10, borderRadius:'50%', background:hex, boxShadow:`0 0 8px ${hex}`, animation:'pulse 2s infinite' }} />
        <div style={{ fontFamily:'Syne, sans-serif', fontWeight:800, fontSize:16, color:hex }}>{cluster.name}</div>
        <div style={{ fontSize:10, color:C.dim, fontFamily:'IBM Plex Mono, monospace' }}>{cluster.url}</div>
        <div style={{ marginLeft:'auto', fontSize:10, color:C.dim, fontFamily:'IBM Plex Mono, monospace' }}>
          v{data?.version || '—'} · {data?.demo ? 'demo' : 'production'}
        </div>
      </div>

      {/* Metrics */}
      <div style={{ display:'grid', gridTemplateColumns:'repeat(4, 1fr)', gap:8, marginBottom:16 }}>
        <MetricCard label="Virtual Machines" value={vms.length} sub={`${running} running`} color={hex} />
        <MetricCard label="Cluster Nodes"    value={nodes.length} sub="connected" color={hex} />
        <MetricCard label="Avg CPU %" value={`${avgCpu}%`} sub="across nodes" color={avgCpu > 70 ? C.amb : hex} />
        <MetricCard label="Avg RAM %" value={`${avgMem}%`} sub="across nodes" color={avgMem > 80 ? C.red : hex} />
      </div>

      {/* CPU History */}
      {history.length > 1 && (
        <Card style={{ marginBottom:16 }}>
          <SectionHeader title="CPU History" />
          <ResponsiveContainer width="100%" height={100}>
            <AreaChart data={history}>
              <defs>
                <linearGradient id="cg2" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor={hex} stopOpacity={0.2}/>
                  <stop offset="95%" stopColor={hex} stopOpacity={0}/>
                </linearGradient>
              </defs>
              <XAxis dataKey="t" hide />
              <YAxis hide domain={[0, 100]} />
              <Tooltip contentStyle={{ background:C.bg3, border:`1px solid ${C.brd}`, fontSize:10 }} />
              <Area type="monotone" dataKey="cpu" stroke={hex} fill="url(#cg2)" strokeWidth={1.5} />
            </AreaChart>
          </ResponsiveContainer>
        </Card>
      )}

      {/* Nodes */}
      {nodes.length > 0 && <>
        <SectionHeader title="Nodes" />
        <div style={{ display:'grid', gridTemplateColumns:'repeat(3, 1fr)', gap:8, marginBottom:16 }}>
          {nodes.map((n, i) => (
            <Card key={i}>
              <div style={{ display:'flex', justifyContent:'space-between', alignItems:'center', marginBottom:10 }}>
                <div style={{ fontSize:12, color:C.txt, fontWeight:500 }}>{n.hostname || `node-${i+1}`}</div>
                <Badge color={C.g}>HEALTHY</Badge>
              </div>
              <div style={{ marginBottom:6 }}>
                <div style={{ display:'flex', justifyContent:'space-between', fontSize:10, color:C.dim, marginBottom:2 }}>
                  <span>CPU</span><span style={{ color:hex }}>{Math.round(n.cpu_usage_pct||0)}%</span>
                </div>
                <Bar pct={n.cpu_usage_pct||0} color={hex} />
              </div>
              <div style={{ marginBottom:8 }}>
                <div style={{ display:'flex', justifyContent:'space-between', fontSize:10, color:C.dim, marginBottom:2 }}>
                  <span>RAM</span><span style={{ color:hex }}>{Math.round(n.mem_usage_pct||0)}%</span>
                </div>
                <Bar pct={n.mem_usage_pct||0} color={hex} />
              </div>
              <div style={{ fontSize:10, color:C.dim }}>
                {n.cpu_cores}c · {Math.round((n.mem_total_mib||0)/1024)}GiB · {n.vm_count||0} VMs
              </div>
            </Card>
          ))}
        </div>
      </>}

      {/* VMs */}
      {vms.length > 0 && <>
        <SectionHeader title={`Virtual Machines (${vms.length})`} />
        <Card>
          {vms.sort((a,b) => (b.cpu||0) - (a.cpu||0)).slice(0, 10).map(vm => (
            <div key={vm.id} style={{ display:'flex', alignItems:'center', gap:12, padding:'8px 0', borderBottom:`1px solid ${C.brd}` }}>
              <StatusDot status={vm.status} />
              <span style={{ width:130, fontSize:12, color:C.txt, fontWeight:500, overflow:'hidden', textOverflow:'ellipsis', whiteSpace:'nowrap' }}>{vm.name}</span>
              <span style={{ width:80, fontSize:10, color:C.dim }}>{vm.cpus}vCPU · {vm.mem_mib}M</span>
              <div style={{ flex:1 }}><Bar pct={vm.cpu||0} color={hex} /></div>
              <span style={{ width:35, fontSize:11, color: (vm.cpu||0) > 80 ? C.red : hex, textAlign:'right' }}>{Math.round(vm.cpu||0)}%</span>
            </div>
          ))}
          {vms.length === 0 && <div style={{ fontSize:11, color:C.dim, textAlign:'center', padding:16 }}>No VMs running</div>}
        </Card>
      </>}

      {vms.length === 0 && nodes.length === 0 && (
        <div style={{ textAlign:'center', color:C.dim, fontSize:12, padding:32 }}>Cluster connected — no data yet</div>
      )}
    </Scroller>
  )
}
