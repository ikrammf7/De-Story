import { create } from 'zustand'

export type Route = { name: 'home' } | { name: 'settings'; back?: Route } | { name: 'project'; id: string }

export interface Toast {
  id: number
  kind: 'error' | 'info' | 'success'
  text: string
  action?: { label: string; run: () => void }
}

interface AppState {
  route: Route
  toasts: Toast[]
  go(route: Route): void
  toast(kind: Toast['kind'], text: string, action?: Toast['action']): void
  dismiss(id: number): void
}

let nextId = 1

export const useApp = create<AppState>((set, get) => ({
  route: { name: 'home' },
  toasts: [],
  go: (route) => set({ route }),
  toast: (kind, text, action) => {
    const id = nextId++
    set({ toasts: [...get().toasts, { id, kind, text, action }].slice(-4) })
    setTimeout(() => get().dismiss(id), kind === 'error' ? 8000 : 4500)
  },
  dismiss: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) })
}))
