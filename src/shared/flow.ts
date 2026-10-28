import type { HfModel } from './higgsfield'
import type { ModelOption } from './types'

export const FLOW_IMAGE_MODELS: HfModel[] = [
  {
    id: 'flow:gemini-3.1-flash-image',
    name: 'Nano Banana 2.1 (Flash)',
    family: 'Google Flow',
    kind: 'image',
    input: {
      field: 'image',
      mode: 'reference',
      max: 3
    },
    required: ['prompt'],
    props: {
      prompt: {
        type: 'string',
        maxLength: 4000
      },
      aspect_ratio: {
        type: 'string',
        enum: ['16:9', '9:16', '1:1', '4:3', '3:4'],
        default: '16:9'
      },
      resolution: {
        type: 'string',
        enum: ['1k', '2k', '4k'],
        default: '2k'
      }
    },
    durations: null,
    tags: ['Rekomendasi', 'Google Flow', 'Referensi', 'Cepat']
  },
  {
    id: 'flow:gemini-3.1-flash-lite-image',
    name: 'Nano Banana 2 Lite',
    family: 'Google Flow',
    kind: 'image',
    input: {
      field: 'image',
      mode: 'reference',
      max: 1
    },
    required: ['prompt'],
    props: {
      prompt: {
        type: 'string',
        maxLength: 4000
      },
      aspect_ratio: {
        type: 'string',
        enum: ['16:9', '9:16', '1:1'],
        default: '16:9'
      }
    },
    durations: null,
    tags: ['Google Flow', 'Ringan', 'Cepat']
  },
  {
    id: 'flow:gemini-3.0-pro-image',
    name: 'Banana Pro',
    family: 'Google Flow',
    kind: 'image',
    input: {
      field: 'image',
      mode: 'reference',
      max: 2
    },
    required: ['prompt'],
    props: {
      prompt: {
        type: 'string',
        maxLength: 4000
      },
      aspect_ratio: {
        type: 'string',
        enum: ['16:9', '9:16', '1:1'],
        default: '16:9'
      }
    },
    durations: null,
    tags: ['Google Flow', 'Pro', 'Artistik']
  }
]

export const FLOW_VIDEO_MODELS: HfModel[] = [
  {
    id: 'flow:veo-3.1-fast',
    name: 'Google Veo 3.1 Fast',
    family: 'Google Flow',
    kind: 'video',
    input: {
      field: 'image',
      mode: 'first-frame',
      max: 1
    },
    required: ['prompt', 'image'],
    props: {
      prompt: {
        type: 'string',
        maxLength: 2500
      },
      resolution: {
        type: 'string',
        enum: ['720p', '1080p'],
        default: '1080p'
      }
    },
    durations: {
      values: [4, 6]
    },
    tags: ['Rekomendasi', 'Google Flow', 'Veo', 'Cepat']
  },
  {
    id: 'flow:veo-3.1-quality',
    name: 'Google Veo 3.1 Quality',
    family: 'Google Flow',
    kind: 'video',
    input: {
      field: 'image',
      mode: 'first-frame',
      max: 1
    },
    required: ['prompt', 'image'],
    props: {
      prompt: {
        type: 'string',
        maxLength: 2500
      },
      resolution: {
        type: 'string',
        enum: ['720p', '1080p'],
        default: '1080p'
      }
    },
    durations: {
      values: [4, 6]
    },
    tags: ['Google Flow', 'Veo', 'Kualitas']
  },
  {
    id: 'flow:veo-3.1-lite',
    name: 'Google Veo 3.1 Lite',
    family: 'Google Flow',
    kind: 'video',
    input: {
      field: 'image',
      mode: 'first-frame',
      max: 1
    },
    required: ['prompt', 'image'],
    props: {
      prompt: {
        type: 'string',
        maxLength: 2000
      },
      resolution: {
        type: 'string',
        enum: ['720p'],
        default: '720p'
      }
    },
    durations: {
      values: [4, 6]
    },
    tags: ['Google Flow', 'Veo', 'Ringan']
  },
  {
    id: 'flow:omni-1.1-flash',
    name: 'Google Omni 1.1 Flash',
    family: 'Google Flow',
    kind: 'video',
    input: {
      field: 'image',
      mode: 'first-frame',
      max: 1
    },
    required: ['prompt', 'image'],
    props: {
      prompt: {
        type: 'string',
        maxLength: 2500
      },
      resolution: {
        type: 'string',
        enum: ['720p', '1080p'],
        default: '1080p'
      }
    },
    durations: {
      values: [4, 6, 8, 10]
    },
    tags: ['Google Flow', 'Omni', 'Dinamis']
  }
]

/** Model ids of retired releases keep resolving so old projects still open. */
export function mapLegacyFlowModel(id: string): string {
  const legacy: Record<string, string> = {
    'flow:nano-banana-2': 'flow:gemini-3.1-flash-image',
    'flow:nano-banana-2-lite': 'flow:gemini-3.1-flash-lite-image',
    'flow:nano-banana-pro': 'flow:gemini-3.0-pro-image',
    'flow:omni-flash': 'flow:omni-1.1-flash'
  }
  return legacy[id] ?? id
}

export function isFlowModel(id: string | null | undefined): boolean {
  return typeof id === 'string' && id.startsWith('flow:')
}

export function flowModelOptions(kind: 'image' | 'video'): ModelOption[] {
  const models = kind === 'image' ? FLOW_IMAGE_MODELS : FLOW_VIDEO_MODELS
  return models.map((m) => ({
    id: m.id,
    name: m.name,
    family: m.family,
    description: `Google Flow (${m.name}) · Tanpa kuota kredit Higgsfield`,
    tags: m.tags,
    credits: 0,
    usd: 0
  }))
}
