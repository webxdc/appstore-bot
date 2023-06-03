import { Component, Show, createMemo } from 'solid-js';
import { FrontendAppInfo } from '../bindings/FrontendAppInfo';
import { useStorage } from 'solidjs-use';
import { ReceivedStatusUpdate } from '../webxdc';
import AppInfoPreview from '../components/AppInfo';
import mock from '../mock'
import { render } from 'solid-js/web';
import '../index.sass';
import "virtual:uno.css"
import '@unocss/reset/tailwind.css'

const Submit: Component = () => {
    const [appInfo, setAppInfo] = useStorage('app-info', {} as FrontendAppInfo)
    const [lastSerial, setlastSerial] = useStorage('last-serial', 0)
    const is_appdata_complete = createMemo(() => Object.values(appInfo()).reduce((init, v) => init && !(v === undefined || v === null || v === ''), true))
    let lastAppinfo: FrontendAppInfo = {} as FrontendAppInfo
    const is_different = createMemo(() => JSON.stringify(appInfo()) !== JSON.stringify(lastAppinfo))
    const has_loaded = createMemo(() => Object.hasOwn(appInfo(), "version"))

    if (import.meta.env.DEV) {
        lastAppinfo = mock
        setAppInfo(mock);
    }

    window.webxdc.setUpdateListener((resp: ReceivedStatusUpdate<FrontendAppInfo>) => {
        setlastSerial(resp.serial)
        // skip events that have a request_type and are hence self-send
        if (!Object.hasOwn(resp.payload, "request_type")) {
            if (!has_loaded()) {
                lastAppinfo = resp.payload
            }
            setAppInfo(resp.payload)
            console.log("Received app info", appInfo())
        }
    }, lastSerial())

    function submit() {
        lastAppinfo = appInfo()
        window.webxdc.sendUpdate({
            payload: {
                request_type: "",
                data: appInfo()
            }
        }, "")
    }

    return (
        <div class="c-grid m-4">
            <div class="min-width flex flex-col gap-3">
                <h1 class="text-2xl text-center font-bold text-indigo-500"> App Metadata</h1>
                <Show when={has_loaded()} fallback={
                    <p>Waiting for setup message...</p>
                }>
                    <AppInfoPreview appinfo={appInfo()} setAppInfo={setAppInfo} />
                    {is_different() && <button class="btn" disabled={!is_appdata_complete()} onclick={submit}> Submit </button>}
                </Show>
            </div>
        </div>
    )
};

const root = document.getElementById('root');
render(() => <Submit />, root!);