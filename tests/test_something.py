import shutil
import subprocess
import base64
import zipfile
from pathlib import Path
from subprocess import Popen

import pytest


class BotProcess:
    addr: str
    process: Popen

    def __init__(self, addr, password, path, binary_path):
        self.addr = addr
        self.path = path

        self.process = Popen(
            [binary_path, "start"],
            cwd=path,
            env={"addr": addr, "mail_pw": password, "RUST_LOG": "xdcstore=trace"},
        )

    def __del__(self):
        self.process.terminate()


@pytest.fixture
def bot_binary_path():
    for path in [
        Path.cwd() / "target" / "debug" / "xdcstore",
        Path.cwd() / "xdcstore" / "xdcstore",
    ]:
        if path.exists():
            return path
    pytest.fail("could not determine bot_binary_path")


@pytest.fixture
def bot_assets_path():
    for path in [
        Path.cwd() / "bot-data",
        Path.cwd() / "xdcstore" / "bot-data",
    ]:
        if path.exists():
            return path
    pytest.fail("could not determine bot_assets_path")


@pytest.fixture
def bot_path(tmp_path, bot_assets_path):
    # Copy bot-data to the bot working directory.
    shutil.copytree(bot_assets_path, tmp_path / "bot-data")

    return tmp_path


@pytest.fixture
def storebot(acfactory, bot_path, bot_binary_path):
    """Store bot without any apps."""
    config = acfactory.get_next_liveconfig()

    return BotProcess(config["addr"], config["mail_pw"], bot_path, bot_binary_path)


@pytest.fixture
def storebot_example(acfactory, bot_path, bot_binary_path):
    """Store bot with imported example apps."""
    config = acfactory.get_next_liveconfig()

    res = subprocess.run(
        [
            bot_binary_path,
            "import",
            Path.cwd() / "example-xdcs",
        ],
        cwd=bot_path,
        env={
            "RUST_LOG": "xdcstore=trace",
            "addr": config["addr"],
            "mail_pw": config["mail_pw"],
        },
    )
    res.check_returncode()

    return BotProcess(config["addr"], config["mail_pw"], bot_path, bot_binary_path)


def test_welcome_message(acfactory, storebot):
    """Test that the bot responds with a welcome message."""
    (ac1,) = acfactory.get_online_accounts(1)

    bot_contact = ac1.create_contact(storebot.addr)
    bot_chat = bot_contact.create_chat()
    bot_chat.send_text("hi!")

    msg_in = ac1.wait_next_incoming_message()
    assert "Welcome to the webxdc store!" in msg_in.text


def test_update(acfactory, storebot):
    """Test that the bot sends initial update and responds to update requests."""
    (ac1,) = acfactory.get_online_accounts(1)

    bot_contact = ac1.create_contact(storebot.addr)
    bot_chat = bot_contact.create_chat()
    bot_chat.send_text("hi!")

    msg_in = ac1.wait_next_incoming_message()
    ac1._evtracker.get_matching("DC_EVENT_WEBXDC_STATUS_UPDATE")
    assert "Welcome" in msg_in.text

    assert msg_in.is_webxdc()
    status_updates = msg_in.get_status_updates()
    assert len(status_updates) == 1
    assert status_updates[0]["payload"] == {
        "type": "Update",
        "app_infos": [],
        "serial": 0,
    }

    # Request updates.
    assert msg_in.send_status_update({"payload": {"Update": {"serial": 0}}}, "update")
    ac1._evtracker.get_matching("DC_EVENT_WEBXDC_STATUS_UPDATE")

    # Receive a response.
    ac1._evtracker.get_matching("DC_EVENT_WEBXDC_STATUS_UPDATE")
    status_updates = msg_in.get_status_updates()
    assert len(status_updates) == 3
    payload = status_updates[-1]["payload"]
    assert payload == {"type": "Update", "app_infos": [], "serial": 0}


def test_import(acfactory, storebot_example):
    """Test that import works."""
    (ac1,) = acfactory.get_online_accounts(1)

    bot_contact = ac1.create_contact(storebot_example.addr)
    bot_chat = bot_contact.create_chat()
    bot_chat.send_text("hi!")

    msg_in = ac1.wait_next_incoming_message()
    ac1._evtracker.get_matching("DC_EVENT_WEBXDC_STATUS_UPDATE")
    assert "Welcome" in msg_in.text

    assert msg_in.is_webxdc()
    status_updates = msg_in.get_status_updates()
    assert len(status_updates) == 1
    payload = status_updates[0]["payload"]
    app_infos = payload["app_infos"]
    assert len(app_infos) == 4


def test_version(acfactory, storebot, bot_path, bot_binary_path):
    """Test /version command."""

    (ac1,) = acfactory.get_online_accounts(1)

    version_text = subprocess.run(
        [bot_binary_path, "version"], cwd=bot_path, capture_output=True, check=True
    ).stdout.decode()

    bot_contact = ac1.create_contact(storebot.addr)
    bot_chat = bot_contact.create_chat()
    bot_chat.send_text("/version")

    msg_in = ac1.wait_next_incoming_message()

    assert msg_in.text + "\n" == version_text


def test_download(acfactory, storebot_example):
    """Test that download works."""
    (ac1,) = acfactory.get_online_accounts(1)

    bot_contact = ac1.create_contact(storebot_example.addr)
    bot_chat = bot_contact.create_chat()
    bot_chat.send_text("hi!")

    msg_in = ac1.wait_next_incoming_message()
    ac1._evtracker.get_matching("DC_EVENT_WEBXDC_STATUS_UPDATE")

    status_updates = msg_in.get_status_updates()
    payload = status_updates[0]["payload"]
    app_infos = payload["app_infos"]
    xdc_2040 = [xdc for xdc in app_infos if xdc["name"] == "2048"][0]

    assert msg_in.send_status_update(
        {"payload": {"Download": {"app_id": xdc_2040["id"]}}}, "update"
    )

    ac1._evtracker.get_matching("DC_EVENT_WEBXDC_STATUS_UPDATE")
    ac1._evtracker.get_matching("DC_EVENT_WEBXDC_STATUS_UPDATE")

    # Test download response for existing app.
    status_updates = msg_in.get_status_updates()
    payload = status_updates[2]["payload"]
    assert payload["type"] == "DownloadOkay"
    assert payload["id"] == xdc_2040["id"]
    assert payload["name"] == "2048"
    with open(str(Path.cwd()) + "/example-xdcs/2048.xdc", "rb") as f:
        assert payload["data"] == base64.b64encode(f.read()).decode("ascii")

    # Test download response for non-existing app.
    assert msg_in.send_status_update({"payload": {"Download": {"app_id": 9}}}, "update")
    ac1._evtracker.get_matching("DC_EVENT_WEBXDC_STATUS_UPDATE")
    ac1._evtracker.get_matching("DC_EVENT_WEBXDC_STATUS_UPDATE")
    status_updates = msg_in.get_status_updates()
    payload = status_updates[4]["payload"]
    assert payload["type"] == "DownloadError"
    assert payload["id"] == 9


def update_manifest_version(bot_path, new_version):
    temp_zip_file = bot_path + "temp.xdc"
    zip_file_path = bot_path + "/bot-data/store.xdc"
    with zipfile.ZipFile(zip_file_path, "r") as zip_read, zipfile.ZipFile(
        temp_zip_file, "w"
    ) as zip_write:
        for file in zip_read.infolist():
            if file.filename != "manifest.toml":
                zip_write.writestr(file, zip_read.read(file.filename))

        manifest_filename = "manifest.toml"
        manifest_content = zip_read.read(manifest_filename).decode("utf-8")

        updated_content = ""
        for line in manifest_content.split("\n"):
            if line.startswith("version ="):
                updated_content += f'version = "{new_version}"\n'
            else:
                updated_content += line + "\n"

        updated_manifest = zipfile.ZipInfo(manifest_filename)
        updated_manifest.compress_type = zipfile.ZIP_DEFLATED
        zip_write.writestr(updated_manifest, updated_content.encode("utf-8"))

    import os

    os.replace(temp_zip_file, zip_file_path)

    print("Version field in manifest.toml updated successfully!")


def test_frontend_update(acfactory, bot_path, bot_binary_path):
    config = acfactory.get_next_liveconfig()
    bot = BotProcess(config["addr"], config["mail_pw"], bot_path, bot_binary_path)
    (ac1,) = acfactory.get_online_accounts(1)

    bot_contact = ac1.create_contact(bot.addr)
    bot_chat = bot_contact.create_chat()
    bot_chat.send_text("hi!")

    msg_in = ac1.wait_next_incoming_message()
    ac1._evtracker.get_matching(
        "DC_EVENT_WEBXDC_STATUS_UPDATE"
    )  # Inital store hydration
    assert msg_in.is_webxdc()

    update_manifest_version(str(bot_path), "1000.0.0")

    # Start the bot again to load the newer store.xdc version
    del bot
    bot = BotProcess(config["addr"], config["mail_pw"], bot_path, bot_binary_path)

    msg_in.send_status_update({"payload": {"Update": {"serial": 0}}}, "")
    ac1._evtracker.get_matching("DC_EVENT_WEBXDC_STATUS_UPDATE")  # Self-sent
    ac1._evtracker.get_matching(
        "DC_EVENT_WEBXDC_STATUS_UPDATE"
    )  # Update needed response

    # Test that the bot sends an outdated response
    status_updates = msg_in.get_status_updates()
    payload = status_updates[2]["payload"]
    assert payload == {"critical": True, "type": "Outdated", "version": "1000.0.0"}

    # In shop.xdc the update button should send this message
    msg_in.send_status_update({"payload": {"type": "UpdateWebxdc"}}, "")

    ac1._evtracker.get_matching("DC_EVENT_WEBXDC_STATUS_UPDATE")  # Self-sent
    ac1._evtracker.get_matching("DC_EVENT_WEBXDC_STATUS_UPDATE")  # Update sent response

    # Test that the bot sends an a download confirmation
    status_updates = msg_in.get_status_updates()
    payload = status_updates[4]["payload"]
    assert payload == {"type": "UpdateSent"}

    # Test that the bots sends a new version of the store
    msg_in = ac1.wait_next_incoming_message()
    assert msg_in.is_webxdc()
