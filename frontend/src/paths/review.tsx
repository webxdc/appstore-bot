import { Component, Show, createMemo } from 'solid-js';
import { FrontendAppInfo } from '../bindings/FrontendAppInfo';
import { useStorage } from 'solidjs-use';
import { ReceivedStatusUpdate } from '../webxdc';
import AppInfoPreview from '../components/AppInfo';

interface TestStatus {
    android: boolean;
    ios: boolean;
    desktop: boolean;
}

interface ReviewStateProps {
    testStatus: TestStatus;
    setTestStatus(appInfo: TestStatus): void;
}

const ReviewState: Component<ReviewStateProps> = (props) => {
    const handleInputChange = (e: Event) => {
        const target = e.target as HTMLInputElement;
        const name = target.name;
        const value = target.checked;
        props.setTestStatus({ ...props.testStatus, [name]: value });
    };

    return (
        <form class="flex flex-col gap-2 p-4 rounded shadow max-width bg-white border">
            <label class="flex items-center">
                <input class="mb-2" type="checkbox" name='android' checked={props.testStatus.android} onClick={handleInputChange} />
                <span class="ml-2">Works on Android</span>
            </label>
            <label class="flex items-center">
                <input class="mb-2" type="checkbox" name='ios' checked={props.testStatus.ios} onClick={handleInputChange} />
                <span class="ml-2">Works on iOS</span>
            </label>
            <label class="flex items-center">
                <input class="mb-2" type="checkbox" name='desktop' checked={props.testStatus.desktop} onClick={handleInputChange} />
                <span class="ml-2">Works on Desktop</span>
            </label>
        </form>
    );
};

const AppDetail: Component = () => {
    const [appInfo, setAppInfo] = useStorage('app-info', {} as FrontendAppInfo)
    const [testStatus, setTestStatus] = useStorage('test-status', { android: false, ios: false, desktop: false })
    const [lastSerial, setlastSerial] = useStorage('last-serial', 0)
    let lastAppinfo: FrontendAppInfo = {} as FrontendAppInfo

    window.webxdc.setUpdateListener((resp: ReceivedStatusUpdate<FrontendAppInfo>) => {
        setlastSerial(resp.serial)

        // skip events that have a request_type and are hence self-send
        if (!Object.hasOwn(resp.payload, "request_type")) {
            lastAppinfo = resp.payload
            setAppInfo(resp.payload)
            console.log("Received app info", appInfo())
        }
    }, lastSerial())

    if (import.meta.env.DEV) {
        setAppInfo({
            name: "Poll",
            description: "Poll app where you can create crazy cool polls. This is a very long description for the pepe.",
            author_name: "Jonas Arndt",
            author_email: "xxde@you.de",
            source_code_url: "https://example.com",
            image: "a",
            version: "1.11",
            id: "hi",
        });
    }

    const is_appdata_complete = createMemo(() => Object.values(appInfo()).reduce((init, v) => init && !(v === undefined || v === null || v === ''), true))
    const is_testing_complete = createMemo(() => testStatus().android && testStatus().ios && testStatus().desktop)
    const is_complete = createMemo(() => is_appdata_complete() && is_testing_complete())

    function submit() {
    }

    return (
        <div class="c-grid m-4">
            <div class="min-width flex flex-col gap-3">
                <h1 class="text-2xl text-center font-bold text-indigo-500"> App Publishing status</h1>
                <Show when={appInfo() !== undefined} fallback={
                    <p>Waiting for setup message...</p>
                }>
                    <AppInfoPreview appinfo={appInfo()} setAppInfo={setAppInfo} />
                    <p class="text-gray-500 font-italic">Testing Status</p>
                    <div>
                        <ReviewState testStatus={testStatus()} setTestStatus={setTestStatus} />
                    </div>
                    <p class="text-gray-500 font-italic">
                        <Show when={!is_complete()} fallback={
                            <p>Ready for publishing!</p>
                        }>
                            <Show when={!is_appdata_complete()}>
                                <p>TODO: Some app data is still missing</p>
                            </Show>
                            <Show when={!is_testing_complete()}>
                                <p>TODO: Testing is incomplete</p>
                            </Show>
                        </Show>
                    </p>
                    <button class="btn font-semibold text-indigo-500 w-full" classList={{ "bg-gray-100 border-gray-500": !is_complete() }}
                        disabled={is_complete()} onClick={submit}>Submit</button>
                </Show>
            </div>
        </div>
    )
};

export default AppDetail;
