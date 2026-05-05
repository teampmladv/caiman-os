import { useApp } from '../store.jsx'
import { C, Card, Badge, Btn, SectionHeader, Scroller } from '../components/UI.jsx'

const statusColor = { OK: C.g, RUNNING: C.blu, WARN: C.amb, ERR: C.red }

export default function Backup() {
  const { backups, showToast } = useApp()
  return (
    <Scroller>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 8, marginBottom: 16 }}>
        {[['Total Policies', backups.length, C.g],['Last Run', 'OK', C.g],['Storage Used', '34 GB', C.blu]].map(([l,v,c]) => (
          <Card key={l}>
            <div style={{ fontSize: 22, fontFamily: 'Syne', fontWeight: 800, color: c }}>{v}</div>
            <div style={{ fontSize: 10, color: C.dim, textTransform: 'uppercase', letterSpacing: '0.08em' }}>{l}</div>
          </Card>
        ))}
      </div>

      <SectionHeader title="Backup Policies" action={<Btn small primary onClick={() => showToast('New backup policy')}>+ New Policy</Btn>} />
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4, marginBottom: 16 }}>
        {backups.map(b => (
          <Card key={b.id}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
              <div style={{ flex: 1 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 4 }}>
                  <span style={{ fontSize: 12, color: C.txt, fontWeight: 500 }}>{b.name}</span>
                  <Badge color={statusColor[b.status] || C.dim}>{b.status}</Badge>
                </div>
                <div style={{ fontSize: 10, color: C.dim }}>
                  <span style={{ marginRight: 12 }}>Schedule: <code style={{ color: C.g }}>{b.schedule}</code></span>
                  <span style={{ marginRight: 12 }}>Retention: {b.retention}</span>
                  <span>Last: {b.last}</span>
                </div>
                <div style={{ fontSize: 10, color: C.dim, marginTop: 2 }}>VMs: {b.vms.join(', ')}</div>
              </div>
              <div style={{ display: 'flex', gap: 6 }}>
                <Btn small onClick={() => showToast('Running backup...')}>Run Now</Btn>
                <Btn small onClick={() => showToast('Edit policy')}>Edit</Btn>
              </div>
            </div>
          </Card>
        ))}
      </div>

      <SectionHeader title="DR Sites" />
      {[{name:'Hetzner FS (primary)',status:'ONLINE',lag:'0ms'},{name:'Contabo VDS (secondary)',status:'ONLINE',lag:'12ms'},{name:'Offsite Cold',status:'SYNCING',lag:'—'}].map(s => (
        <Card key={s.name} style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 4 }}>
          <div style={{ width: 8, height: 8, borderRadius: '50%', background: s.status === 'ONLINE' ? C.g : C.amb }} />
          <span style={{ flex: 1, fontSize: 11, color: C.txt }}>{s.name}</span>
          <Badge color={s.status === 'ONLINE' ? C.g : C.amb}>{s.status}</Badge>
          <span style={{ fontSize: 10, color: C.dim }}>Lag: {s.lag}</span>
        </Card>
      ))}
    </Scroller>
  )
}
