import { useApp } from '../store.jsx'
import { C, Card, Badge, Btn, SectionHeader, Scroller, MetricCard } from '../components/UI.jsx'
import { LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts'

const netData = Array.from({length:20},(_,i)=>({t:i,rx:Math.round(Math.random()*30+15),tx:Math.round(Math.random()*20+8)}))

export default function Network() {
  const { segRules, showToast } = useApp()
  return (
    <Scroller>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 8, marginBottom: 16 }}>
        <MetricCard label="Mbps RX" value="24.3" sub="avg last 5m" />
        <MetricCard label="Mbps TX" value="11.8" sub="avg last 5m" />
        <MetricCard label="P50 Latency" value="8µs" sub="XDP zero-copy" color={C.blu} />
        <MetricCard label="Packets/sec" value="142K" sub="0 drops" color={C.blu} />
      </div>

      <Card style={{ marginBottom: 16 }}>
        <SectionHeader title="Throughput — Last 20 samples" />
        <ResponsiveContainer width="100%" height={120}>
          <LineChart data={netData}>
            <XAxis dataKey="t" hide />
            <YAxis hide />
            <Tooltip contentStyle={{ background: C.bg3, border: `1px solid ${C.brd}`, fontSize: 10 }} />
            <Line type="monotone" dataKey="rx" stroke={C.g} strokeWidth={1.5} dot={false} name="RX Mbps" />
            <Line type="monotone" dataKey="tx" stroke={C.blu} strokeWidth={1.5} dot={false} name="TX Mbps" />
          </LineChart>
        </ResponsiveContainer>
      </Card>

      <SectionHeader title="Microsegmentation Rules" action={<Btn small primary onClick={() => showToast('New rule')}>+ Rule</Btn>} />
      {segRules.map(r => (
        <Card key={r.id} style={{ display: 'flex', alignItems: 'center', gap: 14, marginBottom: 4 }}>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 11, color: C.txt }}>{r.name}</div>
            <div style={{ fontSize: 10, color: C.dim }}>{r.proto} : {r.port} · latency: {r.lat}</div>
          </div>
          <Badge color={r.action === 'ALLOW' ? C.g : C.red}>{r.action}</Badge>
          <Btn small onClick={() => showToast('Edit rule')}>Edit</Btn>
        </Card>
      ))}

      <div style={{ marginTop: 16 }}>
        <SectionHeader title="Virtual Networks" action={<Btn small primary onClick={() => showToast('New vnet')}>+ Network</Btn>} />
        {[{name:'prod-net',cidr:'10.10.0.0/24',vms:5,vlan:'100'},{name:'dev-net',cidr:'10.20.0.0/24',vms:2,vlan:'200'},{name:'ml-net',cidr:'10.30.0.0/24',vms:1,vlan:'300'}].map(n => (
          <Card key={n.name} style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 4 }}>
            <div style={{ flex: 1 }}>
              <div style={{ fontSize: 11, color: C.txt }}>{n.name}</div>
              <div style={{ fontSize: 10, color: C.dim }}>{n.cidr} · VLAN {n.vlan}</div>
            </div>
            <Badge color={C.g}>{n.vms} VMs</Badge>
          </Card>
        ))}
      </div>
    </Scroller>
  )
}
