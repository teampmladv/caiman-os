import { useApp } from '../store.jsx'
import { C, Card, Bar, Badge, SectionHeader, Scroller, MetricCard } from '../components/UI.jsx'

export default function GPU() {
  const { gpus, vms } = useApp()
  const gpuVMs = vms.filter(v => v.gpu)

  return (
    <Scroller>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 8, marginBottom: 16 }}>
        <MetricCard label="GPUs" value={gpus.length} sub="1 assigned" color={C.pur} />
        <MetricCard label="VRAM Total" value="24 GB" sub={`${gpus[0]?.usedVram||0} GB used`} color={C.pur} />
        <MetricCard label="Avg Utilization" value={`${gpus[0]?.util||0}%`} sub="1 active VM" color={gpus[0]?.util > 80 ? C.amb : C.pur} />
      </div>

      <SectionHeader title="GPU Devices" />
      {gpus.map(g => (
        <Card key={g.id} style={{ marginBottom: 8 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 12 }}>
            <div>
              <div style={{ fontSize: 13, color: C.txt, fontWeight: 500 }}>{g.model}</div>
              <div style={{ fontSize: 10, color: C.dim }}>Driver {g.driver} · Node: {g.node}</div>
            </div>
            <Badge color={g.assignedTo ? C.pur : C.dim}>{g.assignedTo ? 'ASSIGNED' : 'FREE'}</Badge>
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 12 }}>
            <div>
              <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 10, color: C.dim, marginBottom: 2 }}><span>GPU Util</span><span style={{ color: C.pur }}>{Math.round(g.util)}%</span></div>
              <Bar pct={g.util} color={C.pur} />
            </div>
            <div>
              <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 10, color: C.dim, marginBottom: 2 }}><span>VRAM</span><span style={{ color: C.pur }}>{g.usedVram}/{g.vram} GB</span></div>
              <Bar pct={Math.round(g.usedVram/g.vram*100)} color={C.pur} />
            </div>
            <div>
              <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 10, color: C.dim, marginBottom: 2 }}><span>Temp</span><span style={{ color: g.temp > 80 ? C.red : C.pur }}>{Math.round(g.temp)}°C</span></div>
              <Bar pct={Math.round(g.temp/100*100)} color={g.temp > 80 ? C.red : C.pur} />
            </div>
          </div>
          {g.assignedTo && <div style={{ marginTop: 8, fontSize: 10, color: C.dim }}>Assigned to: <span style={{ color: C.pur }}>{g.assignedTo}</span></div>}
        </Card>
      ))}

      <SectionHeader title="GPU-Enabled VMs" />
      {gpuVMs.map(vm => (
        <Card key={vm.id} style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 4 }}>
          <div style={{ width: 8, height: 8, borderRadius: '50%', background: C.pur }} />
          <span style={{ flex: 1, fontSize: 11, color: C.txt }}>{vm.name}</span>
          <Badge color={C.pur}>GPU PASSTHROUGH</Badge>
          <span style={{ fontSize: 10, color: C.dim }}>{vm.cpus}vCPU · {vm.mem/1024}GiB · {Math.round(vm.cpu)}%</span>
        </Card>
      ))}
    </Scroller>
  )
}
