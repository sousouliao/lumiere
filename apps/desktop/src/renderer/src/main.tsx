import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import './styles.css'
import { windowsApi } from './tauri-bridge'
import { invoke } from '@tauri-apps/api/core'
import { TrayMenu } from './TrayMenu'

window.lumierePlatform = windowsApi

const rootElement = document.getElementById('root')

if (rootElement === null) {
  throw new Error('Renderer root element was not found.')
}

const isTrayMenu = new URLSearchParams(window.location.search).get('view') === 'tray-menu'
if (isTrayMenu) document.documentElement.dataset.surface = 'tray-menu'

createRoot(rootElement).render(<StrictMode>{isTrayMenu ? <TrayMenu /> : <App />}</StrictMode>)

if (!isTrayMenu) void invoke('renderer_ready')
