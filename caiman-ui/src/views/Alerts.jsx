import { useApp } from '../store.jsx'
import { C, Card, Badge, Btn, SectionHeader, Scroller } from '../components/UI.jsx'

export default function Alerts() {
  const { alerts, toggleAlert, showToast } = useApp()
  return (
    <Scroller>
      <SectionHeader title="Alert Rules" action={<Btn small primary onClick={() => showToast('New alert rule')}>+ New Alert</Btn>} />
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        {alerts.map(a => (
          <Card key={a.id} style={{ display: 'flex', alignItems: 'center', gap: 14, borderLeft: `3px solid ${a.severity === 'crit' ? C.red : a.fired > 0 ? C.amb : C.brd}` }}>
            <div style={{ flex: 1 }}>
              <div style={{ fontSize: 12, color: C.txt, fontWeight: 500 }}>{a.name}</div>
              <div style={{ fontSize: 10, color: C.dim, marginTop: 2 }}>{a.metric} {a.op} {a.threshold}{a.metric === 'cpu' || a.metric === 'mem' ? '%' : ''}</div>
            </div>
            <Badge color={a.severity === 'crit' ? C.red : C.amb}>{a.severity.toUpperCase()}</Badge>
            {a.fired > 0 && <Badge color={C.red}>FIRED {a.fired}x</Badge>}
            <div onClick={() => toggleAlert(a.id)} style={{ width: 36, height: 20, background: a.active ? C.g2 : C.brd, borderRadius: 10, cursor: 'pointer', position: 'relative', transition: 'background 0.2s', flexShrink: 0 }}>
              <div style={{ width: 16, height: 16, background: '#fff', borderRadius: '50%', position: 'absolute', top: 2, left: a.active ? 18 : 2, transition: 'left 0.2s' }} />
            </div>
            <Btn small onClick={() => showToast('Edit alert')}>Edit</Btn>
          </Card>
        ))}
      </div>

      <div style={{ marginTop: 16 }}>
        <SectionHeader title="Notification Channels" />
        {['Slack #ops-alerts', 'PagerDuty', 'Email: team@caimanos.com'].map(ch => (
          <Card key={ch} style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 4 }}>
            <Badge color={C.g}>ACTIVE</Badge>
            <span style={{ fontSize: 11, color: C.txt }}>{ch}</span>
            <Btn small style={{ marginLeft: 'auto' }}>Test</Btn>
          </Card>
        ))}
      </div>
    </Scroller>
  )
}
