import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { StudioApi } from '@shared/api'
import { EVENTS } from '@shared/api'

export const api: StudioApi = {
  projects: {
    list: () => invoke('projects_list'),
    create: (input) => invoke('projects_create', { input }),
    get: (id) => invoke('projects_get', { id }),
    save: (snapshot) => invoke('projects_save', { snap: snapshot }),
    remove: (id) => invoke('projects_remove', { id }),
    duplicate: (id) => invoke('projects_duplicate', { id })
  },
  story: {
    script: (projectId) => invoke('story_script', { projectId }),
    visuals: (projectId, mode) => invoke('story_visuals', { projectId, mode }),
    musicPrompt: (projectId) => invoke('story_music_prompt', { projectId })
  },
  generate: {
    clipImage: (clipId) => invoke('generate_clip_image', { clipId }),
    clipVideo: (clipId) => invoke('generate_clip_video', { clipId }),
    clipTts: (clipId) => invoke('generate_clip_tts', { clipId }),
    characterSheet: (characterId) => invoke('generate_character_sheet', { characterId }),
    missingImages: (projectId) => invoke('generate_missing_images', { projectId }),
    missingVideos: (projectId) => invoke('generate_missing_videos', { projectId }),
    missingTts: (projectId) => invoke('generate_missing_tts', { projectId }),
    narration: (projectId, mode) => invoke('generate_narration', { projectId, mode }),
    syncCaptions: (projectId) => invoke('generate_sync_captions', { projectId }),
    cancel: (jobId) => invoke('generate_cancel', { jobId }),
    estimate: (kind, modelId, aspect, seconds, resolution) =>
      invoke('generate_estimate', { kind, modelId, aspect, seconds, resolution })
  },
  assets: {
    setActive: (clipId, assetId) => invoke('assets_set_active', { clipId, assetId }),
    pickMusic: (projectId) => invoke('assets_pick_music', { projectId }),
    pickImage: (projectId) => invoke('assets_pick_image', { projectId })
  },
  settings: {
    get: () => invoke('settings_get'),
    set: (patch) => invoke('settings_set', { patch }),
    keys: () => invoke('settings_keys'),
    setKey: (provider, key) => invoke('settings_set_key', { provider, key }),
    setGeminiKeys: (keys) => invoke('settings_set_gemini_keys', { keys }),
    addGeminiKeys: (keys) => invoke('settings_add_gemini_keys', { keys }),
    testGeminiKeys: () => invoke('settings_test_gemini_keys'),
    setCustom: (baseUrl, key) => invoke('settings_set_custom', { baseUrl, key }),
    setFlow2api: (baseUrl, key, mode) => invoke('settings_set_flow2api', { baseUrl, key, mode }),
    clearKey: (provider) => invoke('settings_clear_key', { provider }),
    revealKey: (provider) => invoke('settings_reveal_key', { provider }),
    testKey: (provider) => invoke('settings_test_key', { provider }),
    voices: (provider) => invoke('settings_voices', { provider })
  },
  models: {
    list: (source, refresh) => invoke('models_list', { source, refresh })
  },
  exporter: {
    start: (projectId, options) => invoke('export_start', { projectId, opts: options }),
    defaultFolder: () => invoke('export_default_folder'),
    pickFolder: () => invoke('export_pick_folder'),
    reveal: (path) => invoke('export_reveal', { path })
  },
  app: {
    openExternal: (url) => invoke('app_open_external', { url }),
    copyText: (text) => invoke('app_copy_text', { text })
  },
  whisper: {
    status: () => invoke('whisper_status'),
    download: () => invoke('whisper_download'),
    cancel: () => invoke('whisper_cancel'),
    remove: () => invoke('whisper_remove'),
    openFolder: () => invoke('whisper_open_folder')
  },
  flow2api: {
    status: () => invoke('flow2api_status'),
    restart: () => invoke('flow2api_restart'),
    openFolder: () => invoke('flow2api_log_folder'),
    panelInfo: () => invoke('flow2api_panel_info'),
    extensionInfo: () => invoke('flow2api_extension_info'),
    openExtensionFolder: () => invoke('flow2api_extension_folder'),
    setFixedPort: (port) => invoke('flow2api_set_fixed_port', { port })
  },
  on: {
    whisper: (cb) => {
      let unlisten: (() => void) | undefined
      listen(EVENTS.whisper, (e: any) => cb(e.payload)).then((un) => {
        unlisten = un
      })
      return () => unlisten?.()
    },
    flow2api: (cb) => {
      let unlisten: (() => void) | undefined
      listen(EVENTS.flow2api, (e: any) => cb(e.payload)).then((un) => {
        unlisten = un
      })
      return () => unlisten?.()
    },
    job: (cb) => {
      let unlisten: (() => void) | undefined
      listen(EVENTS.job, (e: any) => cb(e.payload)).then((un) => {
        unlisten = un
      })
      return () => unlisten?.()
    },
    clip: (cb) => {
      let unlisten: (() => void) | undefined
      listen(EVENTS.clip, (e: any) => cb(e.payload)).then((un) => {
        unlisten = un
      })
      return () => unlisten?.()
    },
    character: (cb) => {
      let unlisten: (() => void) | undefined
      listen(EVENTS.character, (e: any) => cb(e.payload)).then((un) => {
        unlisten = un
      })
      return () => unlisten?.()
    },
    asset: (cb) => {
      let unlisten: (() => void) | undefined
      listen(EVENTS.asset, (e: any) => cb(e.payload)).then((un) => {
        unlisten = un
      })
      return () => unlisten?.()
    }
  }
}

// Inject Tauri implementation into window.api
if (typeof window !== 'undefined') {
  ;(window as any).api = api
}
