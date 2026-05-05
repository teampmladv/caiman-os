import { useState } from 'react'
import { useApp } from '../store.jsx'
import { C, Card, Bar, StatusDot, Badge, Btn, SectionHeader, Scroller } from '../components/UI.jsx'

export default function VMs() {
  const { vms, nodes, stopVM, startVM, deleteVM, migrateVM, createSnapshot, migrating, showToast } = useApp()
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState('')
  const [dragging, setDragging] = useState(null)
  const [dragOver, setDragOver] = useState(null)
  const [view, setView] = useState('table') // table | topology

  const filtered = vms.filter(v => {
    if (statusFilter && v.status !== statusFilter) return false
    if (search && !v.name.toLowerCase().includes(search.toLowerCase())) return false
    return true
  })

  const onDragStart = (vmId) => setDragging(vmId)
  const onDragOver = (e, nodeId) => { e.preventDefault(); setDragOver(nodeId) }
  const onDrop = (e, nodeId) => {
    e.preventDefault()
    if (dragging) {
      const vm = vms.find(v => v.id === dragging)
      if (vm && vm.node !== nodeId) migrateVM(dragging, nodeId)
      else if (vm && vm.node === nodeId) showToast('VM already on this node', 'info')
    }
    setDragging(null); setDragOver(null)
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {/* Toolbar */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '10px 16px', borderBottom: `1px solid ${C.brd}`, flexShrink: 0, background: C.bg }}>
        <input value={search} onChange={e => setSearch(e.target.value)} placeholder="Search VMs..." style={{ background: C.bg3, border: `1px solid ${C.brd}`, color: C.txt, padding: '5px 10px', fontFamily: 'IBM Plex Mono, monospace', fontSize: 11, width: 180, outline: 'none' }} />
        {['', 'RUNNING', 'STOPPED', 'BOOTING'].map(s => (
          <Btn key={s} small onClick={() => setStatusFilter(s)} style={{ borderColor: statusFilter === s ? C.g : C.brd, color: statusFilter === s ? C.g : C.dim }}>{s || 'All'}</Btn>
        ))}
        <div style={{ flex: 1 }} />
        <Btn small onClick={() => setView(v => v === 'table' ? 'drag' : 'table')} style={{ color: view === 'drag' ? C.g : C.dim, borderColor: view === 'drag' ? C.g : C.brd }}>
          {view === 'drag' ? 'Table View' : 'Migration View'}
        </Btn>
        <Btn primary small onClick={() => showToast('Create VM dialog — coming soon')}>+ Create VM</Btn>
      </div>

      {view === 'table' ? (
        <div style={{ flex: 1, overflowY: 'auto' }}>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 11, tableLayout: 'fixed' }}>
            <thead>
              <tr>
                {[{ label: 'Name', w: 130 }, { label: 'Status', w: 90 }, { label: 'vCPU', w: 50 }, { label: 'RAM', w: 65 }, { label: 'CPU Usage', w: 110 }, { label: 'Node', w: 110 }, { label: 'IP', w: 90 }, { label: 'Tenant', w: 70 }, { label: 'Uptime', w: 80 }, { label: 'Actions', w: 140 }].map(h => (
                  <th key={h.label} style={{ padding: '8px 12px', textAlign: 'left', fontSize: 9, letterSpacing: '0.12em', textTransform: 'uppercase', color: C.dim, borderBottom: `1px solid ${C.brd}`, background: C.bg3, fontWeight: 400, width: h.w }}>{h.label}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {filtered.map(vm => (
                <tr key={vm.id} style={{ borderBottom: `1px solid ${C.brd}`, opacity: migrating === vm.id ? 0.5 : 1 }}>
                  <td style={{ padding: '9px 12px', fontWeight: 500, color: C.txt }}>{vm.name}{vm.gpu && <Badge color={C.pur} style={{ marginLeft: 6 }}>GPU</Badge>}</td>
                  <td style={{ padding: '9px 12px' }}><StatusDot status={vm.status} /><span style={{ color: vm.status === 'RUNNING' ? C.g : vm.status === 'BOOTING' ? C.amb : C.dim, fontSize: 10 }}>{vm.status}</span></td>
                  <td style={{ padding: '9px 12px', color: C.dim }}>{vm.cpus}</td>
                  <td style={{ padding: '9px 12px', color: C.dim }}>{vm.mem}M</td>
                  <td style={{ padding: '9px 12px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                      <div style={{ width: 50, height: 3, background: C.brd, borderRadius: 1 }}>
                        <div style={{ height: '100%', width: `${Math.min(vm.cpu, 100)}%`, background: vm.cpu > 80 ? C.red : vm.cpu > 60 ? C.amb : C.g, borderRadius: 1, transition: 'width 1s ease' }} />
                      </div>
                      <span style={{ fontSize: 10, color: vm.cpu > 80 ? C.red : vm.cpu > 60 ? C.amb : C.g }}>{Math.round(vm.cpu)}%</span>
                    </div>
                  </td>
                  <td style={{ padding: '9px 12px', fontSize: 10, color: C.dim }}>{vm.node.replace('caiman-', '')}</td>
                  <td style={{ padding: '9px 12px', fontSize: 10, color: C.blu }}>{vm.ip}</td>
                  <td style={{ padding: '9px 12px' }}><Badge color={C.pur}>{vm.tenant}</Badge></td>
                  <td style={{ padding: '9px 12px', fontSize: 10, color: C.dim }}>{vm.uptime}</td>
                  <td style={{ padding: '9px 12px' }}>
                    <div style={{ display: 'flex', gap: 4 }}>
                      {vm.status === 'RUNNING' ? <Btn small danger onClick={() => stopVM(vm.id)}>Stop</Btn> : <Btn small onClick={() => startVM(vm.id)}>Start</Btn>}
                      <Btn small onClick={() => createSnapshot(vm.id)}>Snap</Btn>
                      <Btn small danger onClick={() => deleteVM(vm.id)}>Del</Btn>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        /* Live Migration Drag & Drop View */
        <div style={{ flex: 1, overflowY: 'auto', padding: 16 }}>
          <div style={{ fontSize: 11, color: C.dim, marginBottom: 16, background: `${C.blu}12`, border: `1px solid ${C.blu}30`, padding: '8px 12px' }}>
            Drag VMs between nodes to perform live migration. All migrations preserve VM state and memory.
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 12 }}>
            {nodes.map(node => (
              <div key={node.id}
                onDragOver={e => onDragOver(e, node.id)}
                onDrop={e => onDrop(e, node.id)}
                style={{ background: dragOver === node.id ? `${C.g}10` : C.bg2, border: `1px solid ${dragOver === node.id ? C.g : C.brd}`, padding: 12, minHeight: 200, transition: 'all 0.15s', borderRadius: 2 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 10, paddingBottom: 8, borderBottom: `1px solid ${C.brd}` }}>
                  <div>
                    <div style={{ fontSize: 12, color: C.txt, fontWeight: 500 }}>{node.name}</div>
                    <div style={{ fontSize: 10, color: C.dim }}>CPU {Math.round(node.cpu)}% · RAM {Math.round(node.mem)}%</div>
                  </div>
                  {node.gpu && <Badge color={C.pur}>GPU</Badge>}
                </div>
                {vms.filter(v => v.node === node.id).map(vm => (
                  <div key={vm.id} draggable
                    onDragStart={() => onDragStart(vm.id)}
                    style={{ background: migrating === vm.id ? `${C.amb}15` : C.bg3, border: `1px solid ${migrating === vm.id ? C.amb : C.brd}`, padding: '8px 10px', marginBottom: 6, cursor: 'grab', userSelect: 'none', display: 'flex', alignItems: 'center', gap: 8, opacity: migrating === vm.id ? 0.6 : 1 }}>
                    <StatusDot status={vm.status} />
                    <div style={{ flex: 1 }}>
                      <div style={{ fontSize: 11, color: C.txt }}>{vm.name}</div>
                      <div style={{ fontSize: 10, color: C.dim }}>{vm.cpus}vCPU · {vm.mem}M · {Math.round(vm.cpu)}%CPU</div>
                    </div>
                    {migrating === vm.id && <span style={{ fontSize: 9, color: C.amb }}>MIGRATING...</span>}
                  </div>
                ))}
                {dragOver === node.id && dragging && (
                  <div style={{ border: `1px dashed ${C.g}`, padding: '8px 10px', color: C.g, fontSize: 10, textAlign: 'center' }}>
                    Drop to migrate here
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
