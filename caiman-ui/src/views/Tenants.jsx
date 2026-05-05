import { useApp } from '../store.jsx'
import { C, Card, Badge, Btn, SectionHeader, Scroller } from '../components/UI.jsx'

const PERMS = ['vms:read','vms:create','vms:delete','nodes:read','snapshots:*','billing:read','admin:*']

export default function Tenants() {
  const { tenants, showToast } = useApp()
  return (
    <Scroller>
      <SectionHeader title="Tenants" action={<Btn small primary onClick={() => showToast('Create tenant')}>+ New Tenant</Btn>} />
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2,1fr)', gap: 8, marginBottom: 16 }}>
        {tenants.map(t => (
          <Card key={t.id}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 10 }}>
              <div style={{ width: 32, height: 32, borderRadius: '50%', background: t.color+'20', border: `1px solid ${t.color}40`, display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 12, color: t.color, fontWeight: 600 }}>{t.name.slice(0,2).toUpperCase()}</div>
              <div>
                <div style={{ fontSize: 13, color: C.txt, fontWeight: 500 }}>{t.name}</div>
                <div style={{ fontSize: 10, color: C.dim }}>{t.members} members</div>
              </div>
              <Badge color={t.color} style={{ marginLeft: 'auto' }}>{t.role}</Badge>
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 4, fontSize: 10, color: C.dim, paddingTop: 8, borderTop: `1px solid ${C.brd}` }}>
              <div><div style={{ color: t.color, fontSize: 14, fontWeight: 700 }}>{t.vms}</div>VMs</div>
              <div><div style={{ color: t.color, fontSize: 14, fontWeight: 700 }}>{t.vcpus}</div>vCPUs</div>
              <div><div style={{ color: t.color, fontSize: 14, fontWeight: 700 }}>{Math.round(t.ram/1024)}</div>GiB</div>
            </div>
            <div style={{ marginTop: 8 }}>
              <div style={{ fontSize: 9, color: C.dim, marginBottom: 4 }}>QUOTA</div>
              <div style={{ display: 'flex', gap: 4, alignItems: 'center' }}>
                <div style={{ flex: 1, height: 3, background: C.brd, borderRadius: 1 }}>
                  <div style={{ height: '100%', width: `${Math.round(t.vcpus/t.quota_vcpus*100)}%`, background: t.color, borderRadius: 1 }} />
                </div>
                <span style={{ fontSize: 9, color: C.dim }}>{t.vcpus}/{t.quota_vcpus} vCPU</span>
              </div>
            </div>
          </Card>
        ))}
      </div>

      <SectionHeader title="Role Permissions Matrix" />
      <Card>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 10 }}>
          <thead><tr>
            <th style={{ padding: '6px 10px', textAlign: 'left', color: C.dim, borderBottom: `1px solid ${C.brd}`, fontWeight: 400 }}>Permission</th>
            {['Admin','Operator','Developer','Viewer'].map(r => <th key={r} style={{ padding: '6px 10px', color: C.dim, borderBottom: `1px solid ${C.brd}`, fontWeight: 400 }}>{r}</th>)}
          </tr></thead>
          <tbody>
            {PERMS.map(p => (
              <tr key={p} style={{ borderBottom: `1px solid ${C.brd}` }}>
                <td style={{ padding: '6px 10px', color: C.txt }}>{p}</td>
                {[true, p.includes('read')||p.includes('vms:create'), p.includes('read'), p.includes('read')&&!p.includes('billing')].map((has, i) => (
                  <td key={i} style={{ padding: '6px 10px', textAlign: 'center' }}>
                    <span style={{ color: has ? C.g : C.brd }}>{has ? '●' : '○'}</span>
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </Card>
    </Scroller>
  )
}
