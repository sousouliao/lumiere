import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import './styles.css'
import { windowsApi } from './tauri-bridge'
import { invoke } from '@tauri-apps/api/core'

window.lumierePlatform = windowsApi

const rootElement = document.getElementById('root')

if (rootElement === null) {
  throw new Error('Renderer root element was not found.')
}

createRoot(rootElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
)

void invoke('renderer_ready')
