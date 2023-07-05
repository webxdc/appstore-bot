import type { Component } from 'solid-js'
import { Match, Show, Switch, createEffect, createMemo, createSignal } from 'solid-js'
import { For, render } from 'solid-js/web'
import { useStorage } from 'solidjs-use'
import { formatDistance } from 'date-fns'
import Fuse from 'fuse.js'
import { createStore } from 'solid-js/store'
import type { ReceivedStatusUpdate } from './webxdc'
import type { ShopResponse } from './bindings/ShopResponse'
import type { ShopRequest } from './bindings/ShopRequest'
import mock from './mock'
import type { AppInfoWithState, AppInfosById } from './types'
import { AppState } from './types'
import './index.sass'
import 'virtual:uno.css'
import '@unocss/reset/tailwind.css'
import { AppInfoDB } from './db/shop_db'
import OutdatedView from './components/OutdatedView'
import { to_app_infos_by_id, updateHandler } from './shop-logic'

const fuse_options = {
  keys: [
    'name',
    'author_name',
    'description',
  ],
  threshold: 0.4,
}

type DownloadResponseOkay = Extract<ShopResponse, { type: 'DownloadOkay' }>
type UpdateResponse = Extract<ShopResponse, { type: 'Update' }>

function AppInfoModal(item: AppInfoWithState, onDownload: () => void, onForward: () => void, onRemove: () => void) {
  const [isExpanded, setIsExpanded] = createSignal(false)

  return (
    <li class="w-full border rounded p-4 shadow">
      <div class="flex cursor-pointer items-center justify-between gap-2" onClick={() => setIsExpanded(!isExpanded())}>
        <img src={`data:image/png;base64,${item.image!}`} alt={item.name} class="h-20 w-20 rounded-xl object-cover" />
        <div class="flex-grow-1 overflow-hidden">
          <h2 class="text-xl font-semibold">{item.name}</h2>
          <p class="max-width-text truncate text-gray-600">{item.description}</p>
        </div>
        <Switch>
          <Match when={item.state === AppState.Initial}>
            <button class="send-button bg-blue-500" onClick={onDownload}>
              <div class="i-material-symbols:download text-white"></div>
            </button>
          </Match>
          <Match when={item.state === AppState.Downloading}>
            <div class="send-button bg-gray-500">
              <div class="i-eos-icons:bubble-loading text-white"></div>
            </div>
          </Match>
          <Match when={item.state === AppState.DownloadCancelled}>
            <div class="send-button bg-red-500" >
              <div class="i-material-symbols:error text-white"></div>
            </div>
          </Match>
          <Match when={item.state === AppState.Received}>
            <button class="send-button bg-green-500" onClick={onForward}>
              <div class="i-material-symbols:forward text-white"></div>
            </button>
          </Match>
          <Match when={item.state === AppState.Updating}>
            <div class="i-eos-icons:bubble-loading text-black"></div>
            <button class="send-button bg-green-500" onClick={onForward}>
              <div class="i-material-symbols:forward text-white"></div>
            </button>
          </Match>
        </Switch>
      </div >
      {
        isExpanded() && (
          <div class="flex flex-col">
            <p class="my-4 text-gray-600">{item.description}</p>
            <div class="my-2">
              <p class="text-sm text-gray-600"><span class="font-bold"> Submitter: </span>{item.submitter_uri}</p>
              <p class="break-all text-sm text-gray-600"><span class="font-bold"> Source code: </span>{item.source_code_url}</p>
              <p class="text-sm text-gray-600"><span class="font-bold"> Version: </span>{item.version}</p>
            </div>
            {(item.state === AppState.Received || item.state === AppState.Updating) && <button class="self-center btn-gray" onClick={onRemove}>Remove from cache</button>}
          </div>
        )
      }
      <div class="mt-1 flex justify-center" onClick={() => setIsExpanded(!isExpanded())}>
        <button class={`text-blue-800 ${isExpanded() ? 'i-carbon-up-to-top' : 'i-carbon-down-to-bottom'}`}>
        </button>
      </div>
    </li >
  )
}

interface AppListProps {
  items: AppInfoWithState[]
  search: string
  onDownload: (id: string) => void
  onForward: (id: string) => void
  onRemove: (id: string) => void
}

const AppList: Component<AppListProps> = (props) => {
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
    <Show when={props.items.length !== 0} fallback={<p class="text-center unimportant">There are no apps</p>}>
      <For each={filtered_items() || props.items}>
        {
          item => AppInfoModal(item, () => props.onDownload(item.app_id), () => { props.onForward(item.app_id) }, () => props.onRemove(item.app_id))
        }
      </For>
    </Show>
  )
}

const Shop: Component = () => {
  const [appInfo, setAppInfo] = createStore({} as AppInfosById)
  const [lastSerial, setlastSerial] = useStorage('last-serial', 0) // Last store-serial
  const [updateNeeded, setUpdateNeeded] = useStorage('update-needed', false) // Flag if the frontend is outdated
  const [updateReceived, setUpdateReceived] = useStorage('update-received', false)
  const [lastUpdateSerial, setlastUpdateSerial] = useStorage('last-update-serial', 0) // Last serial to initialize updateListener
  const [lastUpdate, setlastUpdate] = useStorage('last-update', new Date())
  const [dateNow, setDateNow] = createSignal(new Date())
  setInterval(() => setDateNow(new Date()), 2000)
  const [isUpdating, setIsUpdating] = createSignal(false)
  const [search, setSearch] = createSignal('')
  const [showCommit, setShowCommit] = createSignal(false) // Show the commit hash when heading was clicked
  const [showCache, setShowCache] = createSignal(false)
  const cached = createMemo(() => Object.values(appInfo).filter(app_info => app_info.state !== AppState.Initial))

  // automatically update the app list
  const past_time = Math.abs(new Date().getTime() - lastUpdate().getTime()) / 1000
  if (appInfo === undefined || (past_time > 60 * 60)) {
    setIsUpdating(true)
  }

  const db = new AppInfoDB('webxdc')
  // This is for now _not_ synchronized with the update receival so a delayed
  // query could overwrite app updates. For now, this should be fine.
  db.get_all().then((apps) => {
    const app_infos = to_app_infos_by_id(apps)
    setAppInfo(app_infos)

    if (import.meta.env.DEV) {
      setAppInfo(to_app_infos_by_id(mock))
      setlastSerial(1)
    }
  })

  window.webxdc.setUpdateListener(async (resp: ReceivedStatusUpdate<UpdateResponse | DownloadResponseOkay>) => {
    updateHandler(resp.payload, db, appInfo, setAppInfo, setlastUpdateSerial, setIsUpdating, setlastUpdate, setUpdateNeeded, setUpdateReceived)
    setlastSerial(resp.serial)
  }, lastSerial())

  async function handleUpdate() {
    setIsUpdating(true)

    const cached_apps = cached().map(app_info => ([app_info.app_id, app_info.version] as [string, number]))
    window.webxdc.sendUpdate({
      payload: { Update: { serial: lastUpdateSerial(), apps: cached_apps } } as ShopRequest,
    }, '')
  }

  async function handleDownload(app_id: string) {
    setAppInfo(app_id, 'state', AppState.Downloading)
    window.webxdc.sendUpdate({
      payload: { Download: { app_id } } as ShopRequest,
    }, '')
  }

  async function handleForward(app_id: string) {
    const file = await db.get_webxdc(app_id)
    if (file === undefined) {
      throw new Error('No cached file found')
    }
    window.webxdc.sendToChat({ file })
  }

  async function handleRemove(app_id: string) {
    setAppInfo(app_id, 'state', AppState.Initial)
    db.remove_webxdc(app_id)
  }

  return (
    <OutdatedView critical={updateNeeded()} updated_received={updateReceived()}>
      <div class="c-grid p-3">
        <div class="min-width">

          {/* header */}
          <div class="flex items-center justify-between gap-2">
            <div>
              <h1 class="flex-shrink text-2xl font-bold" onclick={() => setShowCommit(!showCommit())}>
                Store
              </h1>
              {showCommit() && <p class="whitespace-nowrap text-sm unimportant"> @ {import.meta.env.VITE_COMMIT} </p>}
            </div>
            <div class="rounded-xl p-2 btn-gray">
              <Show when={isUpdating()} fallback={
                <button class="flex items-center gap-2" onclick={handleUpdate}>
                  <span>{formatDistance(lastUpdate(), dateNow())}</span>
                  <div class="border border-blue-500 rounded" i-material-symbols-sync></div>
                </button>
              }>
                <div class="flex items-center gap-2">
                  <span class="tracking-wide">Updating..</span>
                  <div class="loading-spinner border border-blue-500 rounded" i-material-symbols-sync></div>
                </div>
              </Show>
            </div>
          </div>

          {/* app list */}
          <div class="mt-4 p-4">
            <ul class="w-full flex flex-col gap-2">
              <li class="w-full flex flex-col items-center justify-center gap-2">
                <div class="flex items-center justify-center gap-2">
                  <input class="border-2 rounded-2xl" onInput={event => setSearch((event.target as HTMLInputElement).value)} />
                  <button class="rounded-1/2 p-2 btn">
                    <div class="i-carbon-search" />
                  </button>
                </div>
              </li>
              <li class="my-3 flex justify-center gap-3">
                <button class="border-2 btn-gray" classList={{ 'border-none': showCache() }} onClick={() => setShowCache(false)}> All Apps </button>
                <button class="border-2 btn-gray" classList={{ 'border-none': !showCache() }} onClick={() => setShowCache(true)}> Downloaded</button>
              </li>
              <Show when={!(lastSerial() === 0)} fallback={
                <p class="text-center unimportant">Loading store..</p>
              }>
                <AppList
                  items={showCache() ? cached() : Object.values(appInfo)} search={search()}
                  onDownload={handleDownload}
                  onForward={handleForward}
                  onRemove={handleRemove} ></AppList>
              </Show>
            </ul>
          </div>
        </div >
      </div>
    </OutdatedView>
  )
}

const root = document.getElementById('root')
render(() => <Shop />, root!)
