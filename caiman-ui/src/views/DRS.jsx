import { useState } from 'react'
import { useApp } from '../store.jsx'
import { C, Card, Badge, Btn, SectionHeader, Scroller } from '../components/UI.jsx'

export default function DRS() {
  const { nodes, vms, drsRules, showToast } = useApp()
  const [autoBalance, setAutoBalance] = useState(false)
  const avgCpu = nodes.reduce((a,n) => a+n.cpu,0)/nodes.length
  const maxCpu = Math.max(...nodes.map(n=>n.cpu))
  const needsBalance = maxCpu - Math.min(...nodes.map(n=>n.cpu)) > 25

  return (
    <Scroller>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 16 }}>
        <Card>
          <div style={{ fontSize: 10, color: C.dim, marginBottom: 8 }}>DRS STATUS</div>
          <div style={{ fontSize: 22, fontFamily: 'Syne', fontWeight: 800, color: needsBalance ? C.amb : C.g, marginBottom: 4 }}>{needsBalance ? 'IMBALANCED' : 'BALANCED'}</div>
          <div style={{ fontSize: 10, color: C.dim }}>CPU spread: {Math.round(maxCpu - Math.min(...nodes.map(n=>n.cpu)))}% across nodes</div>
          <div style={{ marginTop: 12, display: 'flex', alignItems: 'center', gap: 10 }}>
            <span style={{ fontSize: 11, color: C.dim }}>Auto-balance</span>
            <div onClick={() => setAutoBalance(v => !v)} style={{ width: 36, height: 20, background: autoBalance ? C.g2 : C.brd, borderRadius: 10, cursor: 'pointer', position: 'relative', transition: 'background 0.2s' }}>
              <div style={{ width: 16, height: 16, background: '#fff', borderRadius: '50%', position: 'absolute', top: 2, left: autoBalance ? 18 : 2, transition: 'left 0.2s' }} />
            </div>
            {autoBalance && <Badge color={C.g}>ON</Badge>}
          </div>
        </Card>
        <Card>
          <div style={{ fontSize: 10, color: C.dim, marginBottom: 8 }}>NODE LOAD</div>
          {nodes.map(n => (
            <div key={n.id} style={{ marginBottom: 8 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 10, color: C.dim, marginBottom: 2 }}>
                <span>{n.name.replace('caiman-','')}</span>
                <span style={{ color: n.cpu>80?C.red:n.cpu>60?C.amb:C.g }}>{Math.round(n.cpu)}%</span>
              </div>
              <div style={{ height: 4, background: C.brd, borderRadius: 2 }}>
                <div style={{ height: '100%', width: `${n.cpu}%`, background: n.cpu>80?C.red:n.cpu>60?C.amb:C.g, borderRadius: 2, transition: 'width 0.8s' }} />
              </div>
            </div>
          ))}
          {needsBalance && <Btn primary small style={{ marginTop: 8, width: '100%', justifyContent: 'center' }} onClick={() => showToast('Rebalancing cluster...')}>Rebalance Now</Btn>}
        </Card>
      </div>

      <SectionHeader title="Affinity Rules" action={<Btn small primary onClick={() => showToast('Add rule dialog')}>+ Add Rule</Btn>} />
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        {drsRules.map(r => (
          <Card key={r.id} style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <Badge color={r.type === 'affinity' ? C.g : r.type === 'anti-affinity' ? C.red : r.type === 'cpu-limit' ? C.amb : C.blu}>{r.type}</Badge>
            <div style={{ flex: 1 }}>
              <div style={{ fontSize: 11, color: C.txt }}>{r.constraint}</div>
              <div style={{ fontSize: 10, color: C.dim }}>{r.vms.length > 0 ? r.vms.join(', ') : 'All VMs'}</div>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Badge color={r.active ? C.g : C.dim}>{r.active ? 'ACTIVE' : 'DISABLED'}</Badge>
              <Btn small onClick={() => showToast('Rule edited')}>Edit</Btn>
            </div>
          </Card>
        ))}
      </div>
    </Scroller>
  )
}
