import { useApp } from '../store.jsx'
import { C, Card, Bar, StatusDot, MetricCard, SectionHeader, Scroller, Badge } from '../components/UI.jsx'
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts'

const cpuHistory = Array.from({ length: 20 }, (_, i) => ({ t: i, cpu: Math.round(Math.random() * 30 + 25), net: Math.round(Math.random() * 40 + 10) }))

export default function Dashboard() {
  const { nodes, vms, alerts } = useApp()
  const running = vms.filter(v => v.status === 'RUNNING').length
  const avgCpu  = Math.round(nodes.reduce((a, n) => a + n.cpu, 0) / nodes.length)
  const avgMem  = Math.round(nodes.reduce((a, n) => a + n.mem, 0) / nodes.length)
  const fired   = alerts.filter(a => a.active && a.fired > 0).length

  return (
    <Scroller>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 8, marginBottom: 16 }}>
        <MetricCard label="Virtual Machines" value={vms.length} sub={`${running} running`} />
        <MetricCard label="Cluster Nodes" value={nodes.length} sub="all healthy" />
        <MetricCard label="Avg CPU %" value={avgCpu} sub={`${vms.filter(v=>v.cpu>80).length} VMs high`} color={avgCpu > 70 ? C.amb : C.g} />
        <MetricCard label="Active Alerts" value={fired} sub={fired > 0 ? 'action needed' : 'all clear'} color={fired > 0 ? C.red : C.g} />
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8, marginBottom: 16 }}>
        <Card>
          <SectionHeader title="Cluster CPU History" />
          <ResponsiveContainer width="100%" height={120}>
            <AreaChart data={cpuHistory}>
              <defs><linearGradient id="cg" x1="0" y1="0" x2="0" y2="1"><stop offset="5%" stopColor="#22c55e" stopOpacity={0.2}/><stop offset="95%" stopColor="#22c55e" stopOpacity={0}/></linearGradient></defs>
              <XAxis dataKey="t" hide />
              <YAxis hide domain={[0, 100]} />
              <Tooltip contentStyle={{ background: C.bg3, border: `1px solid ${C.brd}`, fontSize: 10 }} />
              <Area type="monotone" dataKey="cpu" stroke="#22c55e" fill="url(#cg)" strokeWidth={1.5} />
            </AreaChart>
          </ResponsiveContainer>
        </Card>
        <Card>
          <SectionHeader title="Network Throughput" />
          <ResponsiveContainer width="100%" height={120}>
            <AreaChart data={cpuHistory}>
              <defs><linearGradient id="ng" x1="0" y1="0" x2="0" y2="1"><stop offset="5%" stopColor="#60a5fa" stopOpacity={0.2}/><stop offset="95%" stopColor="#60a5fa" stopOpacity={0}/></linearGradient></defs>
              <XAxis dataKey="t" hide />
              <YAxis hide />
              <Tooltip contentStyle={{ background: C.bg3, border: `1px solid ${C.brd}`, fontSize: 10 }} />
              <Area type="monotone" dataKey="net" stroke="#60a5fa" fill="url(#ng)" strokeWidth={1.5} />
            </AreaChart>
          </ResponsiveContainer>
        </Card>
      </div>

      <SectionHeader title="Cluster Nodes" />
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 8, marginBottom: 16 }}>
        {nodes.map(n => (
          <Card key={n.id}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 10 }}>
              <div style={{ fontSize: 12, color: C.txt, fontWeight: 500 }}>{n.name}</div>
              <Badge color={C.g}>HEALTHY</Badge>
            </div>
            <div style={{ marginBottom: 6 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 10, color: C.dim, marginBottom: 2 }}><span>CPU</span><span style={{ color: C.g }}>{Math.round(n.cpu)}%</span></div>
              <Bar pct={n.cpu} />
            </div>
            <div style={{ marginBottom: 8 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 10, color: C.dim, marginBottom: 2 }}><span>RAM</span><span style={{ color: C.g }}>{Math.round(n.mem)}%</span></div>
              <Bar pct={n.mem} />
            </div>
            <div style={{ fontSize: 10, color: C.dim, display: 'flex', gap: 12 }}>
              <span>{n.cores}c · {n.ram}GiB</span>
              <span>{n.vms} VMs</span>
              <span>{n.ip}</span>
              {n.gpu && <Badge color={C.pur}>GPU</Badge>}
            </div>
          </Card>
        ))}
      </div>

      <SectionHeader title="VM Overview — Top 5 by CPU" />
      <Card>
        {vms.sort((a,b) => b.cpu - a.cpu).slice(0, 5).map(vm => (
          <div key={vm.id} style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '8px 0', borderBottom: `1px solid ${C.brd}` }}>
            <StatusDot status={vm.status} />
            <span style={{ width: 120, fontSize: 12, color: C.txt, fontWeight: 500 }}>{vm.name}</span>
            <span style={{ width: 60, fontSize: 10, color: C.dim }}>{vm.cpus}vCPU · {vm.mem}M</span>
            <div style={{ flex: 1 }}><Bar pct={vm.cpu} /></div>
            <span style={{ width: 35, fontSize: 11, color: vm.cpu > 80 ? C.red : vm.cpu > 60 ? C.amb : C.g, textAlign: 'right' }}>{Math.round(vm.cpu)}%</span>
            <span style={{ fontSize: 10, color: C.dim, width: 100 }}>{vm.node.replace('caiman-', '')}</span>
          </div>
        ))}
      </Card>
    </Scroller>
  )
}
