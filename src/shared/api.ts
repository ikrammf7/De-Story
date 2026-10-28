import type {
  ApiProvider,
  AspectRatio,
  AppSettings,
  Asset,
  CharacterPatchEvent,
  ClipPatchEvent,
  CreditEstimate,
  Flow2ApiExtensionInfo,
  Flow2ApiPanelInfo,
  Flow2ApiStatus,
  WhisperStatus,
  ExportOptions,
  Job,
  GeminiKeyTestResult,
  KeyStatus,
  KeyTestResult,
  ModelList,
  ModelSource,
  MusicPrompt,
  NewProjectInput,
  ProjectBundle,
  ProjectSnapshot,
  ProjectSummary,
  TtsProvider,
  VoiceOption
} from './types'

/** The typed surface the preload script exposes as `window.api`. */
export interface StudioApi {
  projects: {
    list(): Promise<ProjectSummary[]>
    create(input: NewProjectInput): Promise<ProjectBundle>
    get(id: string): Promise<ProjectBundle>
    save(snapshot: ProjectSnapshot): Promise<{ updatedAt: number }>
    remove(id: string): Promise<void>
    duplicate(id: string): Promise<ProjectSummary>
  }
  story: {
    /** Idea → scene-by-scene script (story and voice-over). */
    script(projectId: string): Promise<ProjectBundle>
    /** Script → one image plan per scene; "missing" skips scenes whose plan still matches their story. */
    visuals(projectId: string, mode: 'missing' | 'all'): Promise<ProjectBundle>
    /** A background-music prompt that fits the finished script, for use in Suno or another music AI. */
    musicPrompt(projectId: string): Promise<MusicPrompt>
  }
  generate: {
    clipImage(clipId: string): Promise<Job>
    clipVideo(clipId: string): Promise<Job>
    clipTts(clipId: string): Promise<Job>
    characterSheet(characterId: string): Promise<Job>
    missingImages(projectId: string): Promise<Job[]>
    /** An AI video for every clip that has an image but no video yet. */
    missingVideos(projectId: string): Promise<Job[]>
    missingTts(projectId: string): Promise<Job[]>
    /** Voices scenes in one continuous take and cuts it per scene: those without a voice, or all of them. */
    narration(projectId: string, mode: 'missing' | 'all'): Promise<Job>
    /** Re-times caption words of voiced scenes against the audio with the local Whisper model. */
    syncCaptions(projectId: string): Promise<Job>
    cancel(jobId: string): Promise<void>
    /** Credits for one image, or one video of about `seconds`, with this model. Null without a working key. */
    estimate(kind: 'image' | 'video', modelId: string, aspect: AspectRatio, seconds?: number, resolution?: string | null): Promise<CreditEstimate | null>
  }
  assets: {
    setActive(clipId: string, assetId: string): Promise<void>
    pickMusic(projectId: string): Promise<Asset | null>
    /** A logo or watermark image for an overlay. */
    pickImage(projectId: string): Promise<Asset | null>
  }
  settings: {
    get(): Promise<AppSettings>
    set(patch: Partial<AppSettings>): Promise<AppSettings>
    keys(): Promise<KeyStatus[]>
    setKey(provider: ApiProvider, key: string): Promise<KeyTestResult>
    /** Saves and tests the Gemini key pool (one key per entry), replacing the stored pool. */
    setGeminiKeys(keys: string[]): Promise<GeminiKeyTestResult[]>
    /** Appends keys to the Gemini key pool without touching existing ones; each new key is tested. */
    addGeminiKeys(keys: string[]): Promise<GeminiKeyTestResult[]>
    /** Tests every stored Gemini key separately, without rotating. */
    testGeminiKeys(): Promise<GeminiKeyTestResult[]>
    /** Saves an OpenAI-compatible endpoint; the key is optional for local servers. */
    setCustom(baseUrl: string, key: string): Promise<KeyTestResult>
    /** Saves the Flow2API endpoint. `bundled` manages the shipped server itself; `external` talks to the given base URL. */
    setFlow2api(baseUrl: string, key: string, mode: 'bundled' | 'external'): Promise<KeyTestResult>
    clearKey(provider: ApiProvider): Promise<void>
    /** The full stored key, only for the "show key" button. */
    revealKey(provider: ApiProvider): Promise<string | null>
    testKey(provider: ApiProvider): Promise<KeyTestResult>
    voices(provider: TtsProvider): Promise<VoiceOption[]>
  }
  models: {
    /** Live model list from the provider. Served from cache unless `refresh` is set or the cache is stale. */
    list(source: ModelSource, refresh?: boolean): Promise<ModelList>
  }
  exporter: {
    start(projectId: string, options: ExportOptions): Promise<Job>
    defaultFolder(): Promise<string>
    pickFolder(): Promise<string | null>
    reveal(path: string): Promise<void>
  }
  app: {
    openExternal(url: string): Promise<void>
    copyText(text: string): Promise<void>
  }
  whisper: {
    status(): Promise<WhisperStatus>
    download(): Promise<void>
    cancel(): Promise<void>
    remove(): Promise<void>
    openFolder(): Promise<void>
  }
  /** The bundled Flow2API backend: status, restart and the admin panel / extension credentials. */
  flow2api: {
    status(): Promise<Flow2ApiStatus>
    restart(): Promise<Flow2ApiStatus>
    openFolder(): Promise<void>
    panelInfo(): Promise<Flow2ApiPanelInfo>
    extensionInfo(): Promise<Flow2ApiExtensionInfo>
    openExtensionFolder(): Promise<void>
    setFixedPort(port: number): Promise<Flow2ApiStatus>
  }
  on: {
    whisper(cb: (status: WhisperStatus) => void): () => void
    flow2api(cb: (status: Flow2ApiStatus) => void): () => void
    job(cb: (job: Job) => void): () => void
    clip(cb: (e: ClipPatchEvent) => void): () => void
    character(cb: (e: CharacterPatchEvent) => void): () => void
    asset(cb: (asset: Asset) => void): () => void
  }
}

export const EVENTS = {
  job: 'event:job',
  clip: 'event:clip',
  character: 'event:character',
  asset: 'event:asset',
  whisper: 'event:whisper',
  flow2api: 'event:flow2api'
} as const
