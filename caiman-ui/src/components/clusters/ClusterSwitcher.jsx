import { useState, useEffect, useRef } from 'react'
import { loadClusters, getActiveCluster, setActiveCluster, removeCluster } from './ClusterStore.js'
import ConnectClusterModal from './ConnectClusterModal.jsx'
import { C } from '../UI.jsx'

const COLOR_HEX = { cyan:'#00d4ff', green:'#22c55e', amber:'#ffb800', rose:'#ff4466', violet:'#a855f7' }

export default function ClusterSwitcher({ onClusterChange }) {
  const [clusters, setClusters] = useState(loadClusters)
  const [active, setActive]     = useState(getActiveCluster)
  const [open, setOpen]         = useState(false)
  const [showModal, setShowModal] = useState(false)
  const ref = useRef(null)

  useEffect(() => {
    const close = e => { if (ref.current && !ref.current.contains(e.target)) setOpen(false) }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [])

  const select = (cluster) => {
    setActiveCluster(cluster.id); setActive(cluster); setOpen(false)
    onClusterChange?.(cluster)
  }
  const remove = (e, id) => {
    e.stopPropagation(); removeCluster(id)
    const updated = loadClusters(); setClusters(updated)
    if (active?.id === id) { setActive(null); setActiveCluster(null); onClusterChange?.(null) }
  }
  const onConnected = (cluster) => { setClusters(loadClusters()); select(cluster) }

  const hex = active ? (COLOR_HEX[active.color] || '#22c55e') : C.dim

  return (
    <>
      <div ref={ref} style={{ position:'relative', fontFamily:'IBM Plex Mono, monospace' }}>
        <button onClick={() => setOpen(!open)} style={{ display:'flex', alignItems:'center', gap:7, background:C.bg2, border:`1px solid ${C.brd}`, borderRadius:3, padding:'5px 10px', cursor:'pointer', color:C.txt, fontSize:11 }}>
          <span style={{ width:7, height:7, borderRadius:'50%', background:hex, boxShadow:`0 0 5px ${hex}`, flexShrink:0 }} />
          <span style={{ maxWidth:140, overflow:'hidden', textOverflow:'ellipsis', whiteSpace:'nowrap' }}>{active ? active.name : 'No cluster'}</span>
          <span style={{ color:C.dim, fontSize:9 }}>{open ? 'v' : '>'}</span>
        </button>

        {open && (
          <div style={{ position:'absolute', top:'calc(100% + 5px)', right:0, background:'#0d1117', border:`1px solid ${C.brd}`, borderRadius:6, width:280, zIndex:500, boxShadow:'0 12px 40px #000a', overflow:'hidden' }}>
            <div style={{ fontSize:9, color:C.dim, letterSpacing:'0.1em', textTransform:'uppercase', padding:'10px 14px 6px' }}>Clusters</div>
            {clusters.length === 0 && <div style={{ fontSize:11, color:C.dim, padding:'8px 14px 12px', textAlign:'center' }}>No clusters connected</div>}
            {clusters.map(c => {
              const h = COLOR_HEX[c.color] || '#22c55e'
              const isActive = active?.id === c.id
              return (
                <div key={c.id} onClick={() => select(c)}
                  style={{ display:'flex', alignItems:'center', gap:9, padding:'9px 14px', cursor:'pointer', background: isActive ? '#22c55e0a' : 'transparent', borderLeft:`3px solid ${isActive ? h : 'transparent'}` }}>
                  <span style={{ width:7, height:7, borderRadius:'50%', background:h, flexShrink:0 }} />
                  <div style={{ flex:1, minWidth:0 }}>
                    <div style={{ fontSize:12, color:C.txt, overflow:'hidden', textOverflow:'ellipsis', whiteSpace:'nowrap' }}>{c.name}</div>
                    <div style={{ fontSize:10, color:C.dim, overflow:'hidden', textOverflow:'ellipsis', whiteSpace:'nowrap' }}>{c.url.replace(/https?:\/\//, '')}</div>
                  </div>
                  <div style={{ display:'flex', gap:5, alignItems:'center' }}>
                    {isActive && <span style={{ fontSize:9, color:h }}>active</span>}
                    <button onClick={e => remove(e, c.id)} style={{ background:'none', border:'none', color:C.dim, cursor:'pointer', fontSize:11, padding:2 }}>x</button>
                  </div>
                </div>
              )
            })}
            <div style={{ height:1, background:C.brd, margin:'3px 0' }} />
            <button onClick={() => { setOpen(false); setShowModal(true) }}
              style={{ display:'flex', alignItems:'center', gap:8, width:'100%', background:'none', border:'none', color:'#00d4ff', padding:'10px 14px', cursor:'pointer', fontSize:12, textAlign:'left' }}>
              <span style={{ fontSize:16, fontWeight:300 }}>+</span> Connect cluster
            </button>
          </div>
        )}
      </div>

      {showModal && <ConnectClusterModal onClose={() => setShowModal(false)} onConnected={onConnected} />}
    </>
  )
}
