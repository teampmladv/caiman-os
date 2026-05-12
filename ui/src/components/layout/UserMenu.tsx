import React, { useState, useRef, useEffect } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { Settings, LogOut, User, Shield, ChevronDown, Moon } from 'lucide-react'
import { logout } from '../../api/client'

export function UserMenu() {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  const user = localStorage.getItem('caiman_user') ?? 'admin'
  const role = localStorage.getItem('caiman_role') ?? 'admin'

  useEffect(() => {
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', onClick)
    return () => document.removeEventListener('mousedown', onClick)
  }, [])

  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setOpen(o => !o)}
        className="flex items-center gap-1.5 px-2 py-1 rounded border border-caiman-border hover:border-caiman-border2 text-caiman-dim hover:text-caiman-text transition-colors"
      >
        <div className="w-5 h-5 rounded-full bg-caiman-green/20 border border-caiman-green/40 flex items-center justify-center text-[8px] text-caiman-bright uppercase font-mono">
          {user.charAt(0)}
        </div>
        <span className="text-[10px] font-mono tracking-wide">{user}</span>
        <ChevronDown size={9} className={open ? 'rotate-180 transition-transform' : 'transition-transform'} />
      </button>
      <AnimatePresence>
        {open && (
          <motion.div
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ duration: 0.12 }}
            className="absolute right-0 top-full mt-1.5 w-56 bg-caiman-bg2 border border-caiman-border rounded-lg shadow-panel overflow-hidden z-50"
          >
            <div className="px-3 py-2.5 border-b border-caiman-border bg-caiman-bg3">
              <div className="flex items-center gap-2">
                <div className="w-7 h-7 rounded-full bg-caiman-green/20 border border-caiman-green/40 flex items-center justify-center text-[11px] text-caiman-bright uppercase font-mono">
                  {user.charAt(0)}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="text-[11px] text-[#e8f5e9] font-mono truncate">{user}</div>
                  <div className="text-[8px] text-caiman-dim tracking-[1.5px] uppercase flex items-center gap-1">
                    <Shield size={7} /> {role}
                  </div>
                </div>
              </div>
            </div>
            <div className="py-1">
              <MenuItem icon={User}     label="Account"     onClick={() => setOpen(false)} />
              <MenuItem icon={Moon}     label="Theme: Dark" onClick={() => setOpen(false)} />
              <MenuItem icon={Settings} label="Settings"    onClick={() => setOpen(false)} />
            </div>
            <div className="border-t border-caiman-border py-1">
              <MenuItem icon={LogOut} label="Sign out" danger onClick={logout} />
            </div>
            <div className="px-3 py-1.5 border-t border-caiman-border bg-caiman-bg3 text-[8px] text-caiman-dim tracking-[1.5px] flex items-center justify-between">
              <span>HAVANA v0.1.0</span>
              <span>api.caimanos.com</span>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}

function MenuItem({ icon: Icon, label, onClick, danger }: {
  icon: React.ElementType; label: string; onClick: () => void; danger?: boolean
}) {
  return (
    <button
      onClick={onClick}
      className={"w-full flex items-center gap-2 px-3 py-1.5 text-[10px] tracking-wide " + (danger ? "text-caiman-red hover:bg-red-900/20" : "text-caiman-text hover:bg-caiman-bg3 hover:text-[#e8f5e9]")}
    >
      <Icon size={11} className="flex-shrink-0" />
      <span>{label}</span>
    </button>
  )
}
