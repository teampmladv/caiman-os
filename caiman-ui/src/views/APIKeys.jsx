import { useApp } from '../store.jsx'
import { C, Card, Badge, Btn, SectionHeader, Scroller } from '../components/UI.jsx'

export default function APIKeys() {
  const { apiKeys, revokeKey, showToast } = useApp()
  return (
    <Scroller>
      <SectionHeader title="API Keys" action={<Btn small primary onClick={() => showToast('New API key generated')}>+ Generate Key</Btn>} />
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4, marginBottom: 16 }}>
        {apiKeys.map(k => (
          <Card key={k.id} style={{ opacity: k.active ? 1 : 0.5 }}>
            <div style={{ display: 'flex', alignItems: 'flex-start', gap: 14 }}>
              <div style={{ flex: 1 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 4 }}>
                  <span style={{ fontSize: 12, color: C.txt, fontWeight: 500 }}>{k.name}</span>
                  <Badge color={k.active ? C.g : C.dim}>{k.active ? 'ACTIVE' : 'REVOKED'}</Badge>
                </div>
                <div style={{ fontSize: 11, color: C.g, marginBottom: 4 }}>{k.prefix}{'*'.repeat(24)}</div>
                <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap', marginBottom: 4 }}>
                  {k.perms.map(p => <Badge key={p} color={C.blu}>{p}</Badge>)}
                </div>
                <div style={{ fontSize: 10, color: C.dim }}>Created {k.created} · Last used {k.last}</div>
              </div>
              {k.active && <Btn small danger onClick={() => revokeKey(k.id)}>Revoke</Btn>}
            </div>
          </Card>
        ))}
      </div>

      <SectionHeader title="API Usage (last 7 days)" />
      <Card>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 12 }}>
          {[['Total Calls','47.2K'],['Auth Errors','3'],['P50 Latency','12ms'],['Rate Limited','0']].map(([l,v]) => (
            <div key={l} style={{ textAlign: 'center' }}>
              <div style={{ fontSize: 20, fontFamily: 'Syne', fontWeight: 800, color: C.g }}>{v}</div>
              <div style={{ fontSize: 10, color: C.dim }}>{l}</div>
            </div>
          ))}
        </div>
      </Card>
    </Scroller>
  )
}
