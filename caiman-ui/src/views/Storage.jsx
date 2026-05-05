import { useState } from 'react'
import { C, Card, Bar, Badge, Btn, SectionHeader, Scroller } from '../components/UI.jsx'

const POOLS = [
  { name:'nvme-pool-01',total:476,used:180,type:'NVMe RAID-1',node:'caiman-bare-01',iops:'450K' },
  { name:'nvme-pool-02',total:476,used:120,type:'NVMe RAID-1',node:'caiman-node-02',iops:'380K' },
  { name:'ssd-pool-03', total:200,used:80, type:'SSD',       node:'caiman-node-03',iops:'120K' },
  { name:'backup-pool', total:1000,used:340,type:'HDD',      node:'caiman-bare-01',iops:'5K'  },
]
const DISKS = [
  { name:'alpine-root.img',  size:'500 MB', fmt:'ext4',  used:'nginx-prod',  node:'n1' },
  { name:'postgres-data.img',size:'20 GB',  fmt:'qcow2', used:'postgres-01', node:'n1' },
  { name:'worker-os.img',    size:'4 GB',   fmt:'raw',   used:'worker-01,02',node:'n2' },
  { name:'gpu-train.img',    size:'200 GB', fmt:'raw',   used:'gpu-train-01',node:'n1' },
  { name:'backup-2026-05.img',size:'8 GB',  fmt:'raw',   used:'-',           node:'n1' },
]

export default function Storage() {
  const [showCreate, setShowCreate] = useState(false)
  return (
    <Scroller>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 8, marginBottom: 16 }}>
        {[['Total Capacity','2.1 TB',C.g],['Used','720 GB',C.amb],['Available','1.4 TB',C.g],['IOPS Total','955K',C.blu]].map(([l,v,c]) => (
          <Card key={l}>
            <div style={{ fontSize: 22, fontFamily: 'Syne', fontWeight: 800, color: c }}>{v}</div>
            <div style={{ fontSize: 10, color: C.dim, textTransform: 'uppercase', letterSpacing: '0.08em' }}>{l}</div>
          </Card>
        ))}
      </div>

      <SectionHeader title="Storage Pools" action={<Btn small primary onClick={() => setShowCreate(v=>!v)}>+ New Volume</Btn>} />
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2,1fr)', gap: 8, marginBottom: 16 }}>
        {POOLS.map(p => {
          const pct = Math.round(p.used/p.total*100)
          return (
            <Card key={p.name}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 10 }}>
                <div>
                  <div style={{ fontSize: 12, color: C.txt, fontWeight: 500 }}>{p.name}</div>
                  <div style={{ fontSize: 10, color: C.dim }}>{p.type} · {p.node.replace('caiman-','')} · {p.iops} IOPS</div>
                </div>
                <div style={{ width: 48, height: 48, position: 'relative' }}>
                  <svg width="48" height="48" viewBox="0 0 48 48" style={{ transform: 'rotate(-90deg)' }}>
                    <circle cx="24" cy="24" r="20" fill="none" stroke={C.brd} strokeWidth="5"/>
                    <circle cx="24" cy="24" r="20" fill="none" stroke={pct>85?C.red:pct>70?C.amb:C.g} strokeWidth="5" strokeDasharray={`${2*Math.PI*20*pct/100} ${2*Math.PI*20}`} strokeLinecap="round"/>
                  </svg>
                  <div style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 10, color: C.g, fontWeight: 500 }}>{pct}%</div>
                </div>
              </div>
              <div style={{ fontSize: 10, color: C.dim }}>{p.used} / {p.total} GB</div>
            </Card>
          )
        })}
      </div>

      <SectionHeader title="Disk Images" />
      <div style={{ overflowX: 'auto' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 11, tableLayout: 'fixed' }}>
          <thead><tr>
            {['Image Name','Size','Format','Used By','Node','Actions'].map(h => <th key={h} style={{ padding: '8px 12px', textAlign: 'left', fontSize: 9, letterSpacing: '0.12em', textTransform: 'uppercase', color: C.dim, borderBottom: `1px solid ${C.brd}`, background: C.bg3, fontWeight: 400 }}>{h}</th>)}
          </tr></thead>
          <tbody>
            {DISKS.map(d => (
              <tr key={d.name} style={{ borderBottom: `1px solid ${C.brd}` }}>
                <td style={{ padding: '8px 12px', color: C.txt, fontWeight: 500 }}>{d.name}</td>
                <td style={{ padding: '8px 12px', color: C.dim }}>{d.size}</td>
                <td style={{ padding: '8px 12px' }}><Badge color={C.blu}>{d.fmt}</Badge></td>
                <td style={{ padding: '8px 12px', color: C.dim, fontSize: 10 }}>{d.used}</td>
                <td style={{ padding: '8px 12px', color: C.dim, fontSize: 10 }}>{d.node}</td>
                <td style={{ padding: '8px 12px' }}><div style={{ display: 'flex', gap: 4 }}><Btn small>Clone</Btn><Btn small>Snap</Btn></div></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Scroller>
  )
}
