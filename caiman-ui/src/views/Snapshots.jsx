import { useApp } from '../store.jsx'
import { C, Card, Badge, Btn, SectionHeader, Scroller } from '../components/UI.jsx'

export default function Snapshots() {
  const { snapshots, vms, deleteSnapshot, createSnapshot, showToast } = useApp()
  return (
    <Scroller>
      <SectionHeader title="Snapshots" action={<div style={{ display: 'flex', gap: 8 }}>{vms.filter(v=>v.status==='RUNNING').slice(0,3).map(v => <Btn key={v.id} small primary onClick={() => createSnapshot(v.id)}>Snap {v.name}</Btn>)}</div>} />
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        {snapshots.map(s => (
          <Card key={s.id} style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
            <div style={{ width: 8, height: 8, borderRadius: '50%', background: C.g, flexShrink: 0 }} />
            <div style={{ flex: 1 }}>
              <div style={{ fontSize: 12, color: C.txt, fontWeight: 500 }}>{s.vm} <span style={{ fontSize: 10, color: C.dim }}>— {s.desc}</span></div>
              <div style={{ fontSize: 10, color: C.dim, marginTop: 2 }}>{s.created} · {s.size}</div>
            </div>
            <Badge color={C.g}>{s.status}</Badge>
            <div style={{ display: 'flex', gap: 6 }}>
              <Btn small onClick={() => showToast('Restoring snapshot...')}>Restore</Btn>
              <Btn small danger onClick={() => deleteSnapshot(s.id)}>Delete</Btn>
            </div>
          </Card>
        ))}
      </div>

      <div style={{ marginTop: 20 }}>
        <SectionHeader title="Snapshot Timeline" />
        <Card>
          <div style={{ position: 'relative', paddingLeft: 20 }}>
            <div style={{ position: 'absolute', left: 0, top: 0, bottom: 0, width: 1, background: C.brd }} />
            {snapshots.map((s, i) => (
              <div key={s.id} style={{ paddingBottom: 16, paddingLeft: 16, position: 'relative' }}>
                <div style={{ position: 'absolute', left: -4, top: 4, width: 8, height: 8, borderRadius: '50%', background: C.g, border: `2px solid ${C.bg2}` }} />
                <div style={{ fontSize: 11, color: C.txt }}>{s.vm} — {s.desc}</div>
                <div style={{ fontSize: 10, color: C.dim }}>{s.created} · {s.size}</div>
              </div>
            ))}
          </div>
        </Card>
      </div>
    </Scroller>
  )
}
