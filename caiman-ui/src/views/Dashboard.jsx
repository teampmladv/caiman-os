import { useState, useEffect } from 'react'
import { useApp } from '../store.jsx'
import { C, Card, Bar, StatusDot, MetricCard, SectionHeader, Scroller, Badge } from '../components/UI.jsx'
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts'

export default function Dashboard() {
  const { nodes, vms, alerts, isLive } = useApp()
  const [history, setHistory] = useState(
    Array.from({ length: 20 }, (_, i) => ({
      t: i,
      cpu: Math.round(Math.random() * 30 + 25),
      net: Math.round(Math.random() * 40 + 10),
    }))
  )

  // Accumulate real CPU history from live data
  useEffect(() => {
    if (nodes.length === 0) return
    const avgCpuNow = Math.round(nodes.reduce((a, n) => a + (n.cpu || 0), 0) / nodes.length)
    const netNow    = Math.round(nodes.reduce((a, n) => a + (n.net_rx_mbps || n.net || 0), 0))
    setHistory(h => {
      const next = [...h.slice(-19), { t: h.length, cpu: avgCpuNow, net: netNow }]
      return next
    })
  }, [nodes])

  const running  = vms.filter(v => v.status === 'RUNNING' || v.status === 'running').length
  const avgCpu   = nodes.length ? Math.round(nodes.reduce((a, n) => a + (n.cpu || 0), 0) / nodes.length) : 0
  const fired    = alerts.filter(a => a.active && a.fired > 0).length
  const highCpu  = vms.filter(v => (v.cpu_usage_pct || v.cpu || 0) > 80).length

  // Normalize VM fields for display
  const normalVms = vms.map(v => ({
    ...v,
    cpu:  v.cpu_usage_pct || v.cpu || 0,
    mem:  v.mem_mib       || v.mem || 0,
    node: v.node_name     || v.node || '—',
  }))

  return (
    <Scroller>
      {/* Live indicator */}
      {isLive && (
        <div style={{ display:'flex', alignItems:'center', gap:6, marginBottom:12, fontSize:10, color:C.g, fontFamily:'IBM Plex Mono, monospace' }}>
          <span style={{ width:6, height:6, borderRadius:'50%', background:C.g, animation:'pulse 2s infinite', display:'inline-block' }} />
          Live data from cluster
        </div>
      )}

      {/* Metric cards */}
      <div style={{ display:'grid', gridTemplateColumns:'repeat(4, 1fr)', gap:8, marginBottom:16 }}>
        <MetricCard label="Virtual Machines" value={vms.length}    sub={`${running} running`} />
        <MetricCard label="Cluster Nodes"    value={nodes.length}  sub={isLive ? 'connected' : 'simulated'} />
        <MetricCard label="Avg CPU %"        value={`${avgCpu}%`}  sub={`${highCpu} VMs high`} color={avgCpu > 70 ? C.amb : C.g} />
        <MetricCard label="Active Alerts"    value={fired}         sub={fired > 0 ? 'action needed' : 'all clear'} color={fired > 0 ? C.red : C.g} />
      </div>

      {/* Charts */}
      <div style={{ display:'grid', gridTemplateColumns:'1fr 1fr', gap:8, marginBottom:16 }}>
        <Card>
          <SectionHeader title={`Cluster CPU History ${isLive ? '(live)' : '(demo)'}`} />
          <ResponsiveContainer width="100%" height={120}>
            <AreaChart data={history}>
              <defs>
                <linearGradient id="cg" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%"  stopColor="#22c55e" stopOpacity={0.2}/>
                  <stop offset="95%" stopColor="#22c55e" stopOpacity={0}/>
                </linearGradient>
              </defs>
              <XAxis dataKey="t" hide />
              <YAxis hide domain={[0, 100]} />
              <Tooltip contentStyle={{ background:C.bg3, border:`1px solid ${C.brd}`, fontSize:10 }}
                formatter={(v) => [`${v}%`, 'CPU']} />
              <Area type="monotone" dataKey="cpu" stroke="#22c55e" fill="url(#cg)" strokeWidth={1.5} dot={false} />
            </AreaChart>
          </ResponsiveContainer>
        </Card>
        <Card>
          <SectionHeader title="Network Throughput" />
          <ResponsiveContainer width="100%" height={120}>
            <AreaChart data={history}>
              <defs>
                <linearGradient id="ng" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%"  stopColor="#60a5fa" stopOpacity={0.2}/>
                  <stop offset="95%" stopColor="#60a5fa" stopOpacity={0}/>
                </linearGradient>
              </defs>
              <XAxis dataKey="t" hide />
              <YAxis hide />
              <Tooltip contentStyle={{ background:C.bg3, border:`1px solid ${C.brd}`, fontSize:10 }}
                formatter={(v) => [`${v} Mbps`, 'Net']} />
              <Area type="monotone" dataKey="net" stroke="#60a5fa" fill="url(#ng)" strokeWidth={1.5} dot={false} />
            </AreaChart>
          </ResponsiveContainer>
        </Card>
      </div>

      {/* Nodes */}
      <SectionHeader title="Cluster Nodes" />
      <div style={{ display:'grid', gridTemplateColumns:'repeat(3, 1fr)', gap:8, marginBottom:16 }}>
        {nodes.length === 0 && (
          <div style={{ gridColumn:'1/-1', textAlign:'center', padding:32, color:C.dim, fontSize:12 }}>
            {isLive ? 'No nodes reported by cluster' : 'Connect a cluster to see real nodes'}
          </div>
        )}
        {nodes.map((n, i) => {
          const cpu = Math.round(n.cpu || 0)
          const mem = Math.round(n.mem || 0)
          return (
            <Card key={n.id || i}>
              <div style={{ display:'flex', justifyContent:'space-between', alignItems:'center', marginBottom:10 }}>
                <div style={{ fontSize:12, color:C.txt, fontWeight:500 }}>{n.name}</div>
                <Badge color={C.g}>HEALTHY</Badge>
              </div>
              <div style={{ marginBottom:6 }}>
                <div style={{ display:'flex', justifyContent:'space-between', fontSize:10, color:C.dim, marginBottom:2 }}>
                  <span>CPU</span>
                  <span style={{ color: cpu > 80 ? C.red : cpu > 60 ? C.amb : C.g }}>{cpu}%</span>
                </div>
                <Bar pct={cpu} />
              </div>
              <div style={{ marginBottom:8 }}>
                <div style={{ display:'flex', justifyContent:'space-between', fontSize:10, color:C.dim, marginBottom:2 }}>
                  <span>RAM</span>
                  <span style={{ color: mem > 80 ? C.red : mem > 60 ? C.amb : C.g }}>{mem}%</span>
                </div>
                <Bar pct={mem} />
              </div>
              <div style={{ fontSize:10, color:C.dim, display:'flex', gap:12, flexWrap:'wrap' }}>
                {n.cores > 0 && <span>{n.cores}c · {n.ram}GiB</span>}
                <span>{n.vms} VMs</span>
                {n.ip && n.ip !== '—' && <span>{n.ip}</span>}
                {n.gpu && <Badge color={C.pur}>GPU</Badge>}
              </div>
            </Card>
          )
        })}
      </div>

      {/* Top VMs */}
      <SectionHeader title={`VM Overview — Top ${Math.min(normalVms.length, 5)} by CPU`} />
      <Card>
        {normalVms.length === 0 && (
          <div style={{ textAlign:'center', padding:24, color:C.dim, fontSize:12 }}>
            No VMs running
          </div>
        )}
        {[...normalVms].sort((a,b) => b.cpu - a.cpu).slice(0, 5).map(vm => (
          <div key={vm.id} style={{ display:'flex', alignItems:'center', gap:12, padding:'8px 0', borderBottom:`1px solid ${C.brd}` }}>
            <StatusDot status={vm.status?.toUpperCase()} />
            <span style={{ width:130, fontSize:12, color:C.txt, fontWeight:500, overflow:'hidden', textOverflow:'ellipsis', whiteSpace:'nowrap' }}>{vm.name}</span>
            <span style={{ width:70, fontSize:10, color:C.dim }}>{vm.cpus}vCPU · {vm.mem}M</span>
            <div style={{ flex:1 }}><Bar pct={vm.cpu} /></div>
            <span style={{ width:35, fontSize:11, color: vm.cpu > 80 ? C.red : vm.cpu > 60 ? C.amb : C.g, textAlign:'right' }}>{Math.round(vm.cpu)}%</span>
            <span style={{ fontSize:10, color:C.dim, width:100, overflow:'hidden', textOverflow:'ellipsis', whiteSpace:'nowrap' }}>{String(vm.node).replace('caiman-','')}</span>
          </div>
        ))}
      </Card>
    </Scroller>
  )
}
