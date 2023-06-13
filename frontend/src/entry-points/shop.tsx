import type { Component } from 'solid-js'
import { Show, createEffect, createMemo, createSignal } from 'solid-js'
import { For, render } from 'solid-js/web'
import { useStorage } from 'solidjs-use'
import { format } from 'date-fns'
import Fuse from 'fuse.js'
import { createStore, produce } from 'solid-js/store'
import type { ReceivedStatusUpdate } from '../webxdc'
import type { DownloadResponse } from '../bindings/DownloadResponse'
import type { UpdateResponse } from '../bindings/UpdateResponse'
import type { ShopRequest } from '../bindings/ShopRequest'
import mock from '../mock'
import type { AppInfoWithState, AppInfosById } from '../types'
import { AppState } from '../types'
import '../index.sass'
import 'virtual:uno.css'
import '@unocss/reset/tailwind.css'
import { AppInfoDB } from '../db/shop_db'
import type { AppInfo } from '../bindings/AppInfo'

const fuse_options = {
  keys: [
    'name',
    'author_name',
  ],
}

function isEmpty(obj: any) {
  for (const prop in obj) {
    if (Object.prototype.hasOwnProperty.call(obj, prop))
      return false
  }
  return true
}

function isDownloadResponse(p: any): p is DownloadResponse {
  return Object.hasOwn(p, 'okay')
}

function isUpdateResponse(p: any): p is UpdateResponse {
  return Object.hasOwn(p, 'app_infos')
}

function AppInfoModal(item: AppInfoWithState, onDownload: () => void) {
  const [isExpanded, setIsExpanded] = createSignal(false)

  return (
    <li class="w-full border rounded p-4 shadow">
      <div class="flex items-center justify-between gap-2">
        <img src={`data:image/png;base64,${item.image!}`} alt={item.name} class="h-20 w-20 rounded-xl object-cover" />
        <div class="flex-grow-1 overflow-hidden">
          <h2 class="text-xl font-semibold">{item.name}</h2>
          <p class="max-width-text truncate text-gray-600">{item.description}</p>
        </div>
        {item.state === AppState.Initial && <button class="justify-self-center btn" onClick={onDownload}> Add </button>}
        {item.state === AppState.Downloading && <p class="unimportant"> Downloading.. </p>}
        {item.state === AppState.DownloadCancelled && <p class="text-red"> Download cancelled </p>}
        {item.state === AppState.Received && <p class="font-bold text-amber-400"> Received in Chat </p>}
      </div>
      {
        isExpanded() && (
          <>
            <p class="my-2 text-gray-600">{item.description}</p>
            <hr />
            <div class="my-2">
              <p class="text-sm text-gray-600"><span class="font-bold"> author:</span> {item.author_name}</p>
              <p class="text-sm text-gray-600"><span class="font-bold"> contact:</span>  {item.author_email}</p>
              <p class="text-sm text-gray-600"><span class="font-bold"> source code:</span>  {item.source_code_url}</p>
            </div>
          </>
        )
      }
      <div class="flex justify-center">
        <button onClick={() => setIsExpanded(!isExpanded())} class={`text-indigo-500 ${isExpanded() ? 'i-carbon-up-to-top' : 'i-carbon-down-to-bottom'}`}>
        </button>
      </div>
    </li >
  )
}

const PublishButton: Component = () => {
  const [isOpen, setIsOpen] = createSignal(false)

  return (
    <button onClick={() => setIsOpen(true)} class="w-full btn">
      {isOpen() ? 'You can send me your webxdc in our 1:1 chat and I will help you publish it.' : 'Publish your own app'}
    </button>
  )
}

const AppList: Component<{ items: AppInfoWithState[]; search: string; onDownload: (id: number) => void }> = (props) => {
  let fuse: Fuse<AppInfoWithState> = new Fuse(props.items, fuse_options)

  createEffect(() => {
    fuse = new Fuse(props.items, fuse_options)
  })

  const filtered_items = createMemo(() => {
    if (props.search !== '') {
      return fuse!.search(props.search).map(fr => fr.item)
    }
    else {
      return props.items
    }
  })

  return (
    <Show when={props.items.length !== 0} fallback={<p class="text-center unimportant">Loading Apps..</p>}>
      <For each={filtered_items() || props.items}>
        {
          item => AppInfoModal(item, () => { props.onDownload(item.id) })
        }
      </For>
    </Show>
  )
}

function to_app_infos_by_id(app_infos: AppInfo[]): AppInfosById {
  return app_infos.reduce((acc, appinfo) => {
    const index = appinfo.id
    acc[index] = { ...appinfo, state: AppState.Initial }
    return acc
  }, {} as AppInfosById)
}

const Shop: Component = () => {
  const [appInfo, setAppInfo] = createStore({} as AppInfosById)
  const [lastSerial, setlastSerial] = useStorage('last-serial', 0)
  const [lastUpdateSerial, setlastUpdateSerial] = useStorage('last-update-serial', 0)
  const [lastUpdate, setlastUpdate] = useStorage('last-update', new Date())
  const [isUpdating, setIsUpdating] = createSignal(false)
  const [search, setSearch] = createSignal('')

  if (import.meta.env.DEV) {
    setAppInfo(mock.id, mock)
  }

  if (appInfo === undefined) {
    setIsUpdating(true)
  }

  const db = new AppInfoDB('webxdc')

  // This is for now _not_ synchronized with the update receival so a delayed
  // query could overwrite app updates. For now, this should be fine.
  db.get_all().then((apps) => {
    const app_infos = to_app_infos_by_id(apps)
    if (apps.length > 0) {
      setAppInfo(app_infos)
    }
  })

  window.webxdc.setUpdateListener(async (resp: ReceivedStatusUpdate<UpdateResponse | DownloadResponse>) => {
    setlastSerial(resp.serial)
    // Skip events that have a request_type and are hence self-send
    if (isUpdateResponse(resp.payload)) {
      console.log('Received Update')
      const app_infos = to_app_infos_by_id(resp.payload.app_infos)

      if (isEmpty(appInfo)) {
        // initially write the newest update to state
        setAppInfo(app_infos)
        db.insertMultiple(resp.payload.app_infos)
      }
      else {
        // all but the first update only overwrite existing properties
        console.log('Reconceiling updates')
        setAppInfo(produce((s) => {
          for (const key in app_infos) {
            const num_key = Number(key)
            if (s[num_key] === undefined) {
              s[num_key] = app_infos[num_key]
            }
            else {
              s[num_key] = Object.assign(s[num_key], app_infos[num_key])
            }
          }
        }))
        db.updateMultiple(resp.payload.app_infos)
      }

      setlastUpdateSerial(resp.payload.serial)
      setIsUpdating(false)
      setlastUpdate(new Date())
    }
    else if (isDownloadResponse(resp.payload)) {
      if (resp.payload.okay) {
        // id is set if resp is okay
        setAppInfo(resp.payload.id, 'state', AppState.Received)
      }
      else {
        // id is set if resp is okay
        setAppInfo(resp.payload.id, 'state', AppState.DownloadCancelled)
      }
    }
  }, lastSerial())

  async function handleUpdate() {
    setIsUpdating(true)
    window.webxdc.sendUpdate({
      payload: { Update: { serial: lastUpdateSerial() } } as ShopRequest,
    }, '')
  }

  function handleDownload(app_id: number) {
    setAppInfo(Number(app_id), 'state', AppState.Downloading)
    window.webxdc.sendUpdate({
      payload: { Download: { app_id } } as ShopRequest,
    }, '')
  }

  return (
    <div class="c-grid p-3">
      <div class="min-width">
        <div class="flex justify-between gap-2">
          <h1 class="text-2xl font-bold">Webxdc Appstore</h1>
          <div class="flex items-center gap-2 p-1 unimportant">
            <Show when={isUpdating()} fallback={
              <button onclick={handleUpdate}>
                <span>{format(lastUpdate(), 'cccc HH:mm')}</span>
              </button>
            }>
              Updating..
            </Show>
            <div class="border border-indigo-500 rounded" classList={{ 'loading-spinner': isUpdating() }} i-carbon-reset></div>
          </div>
        </div>

        <div class="mt-5 p-4">
          <ul class="w-full flex flex-col gap-2">
            <li class="mb-3 w-full flex items-center justify-center gap-2">
              <input class="border-2 rounded-2xl" onInput={event => setSearch((event.target as HTMLInputElement).value)} />
              <button class="rounded-1/2 p-2 btn">
                <div class="i-carbon-search text-indigo-500" />
              </button>
            </li>
            <AppList items={Object.values(appInfo)} search={search()} onDownload={handleDownload} ></AppList>
            <li class="mt-3">
              <PublishButton></PublishButton>
            </li>
          </ul>
        </div>
      </div >
    </div>
  )
}

const root = document.getElementById('root')
render(() => <Shop />, root!)