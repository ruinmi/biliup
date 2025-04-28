import logging
import re

from .engine.decorators import Plugin
from .plugins import general
from .app import context

logger = logging.getLogger('biliup')


def download(fname, url, **kwargs):
    pg = None
    override = kwargs.get('override', {})
    for plugin in Plugin.download_plugins:
        if re.match(plugin.VALID_URL_BASE, url):
            pg = plugin(fname, url)
            for k in pg.__dict__:
                if kwargs.get(k):
                    pg.__dict__[k] = kwargs.get(k)
    if not pg:
        pg = general.__plugin__(fname, url)
        logger.warning(f'Not found plugin for {fname} -> {url} This may cause problems')
    if override:
        if pg.__class__ in Plugin.download_plugins:
            # 单独适配的plugin允许全覆写
            pg.__dict__.update(override)
            if override.get('user'):
                pg.__dict__.pop('user')
                pg.__dict__.update(override.get('user'))
        else:
            # print("Override General plugin")
            # 通用插件只允许覆写插件存在的值
            for k in pg.__dict__:
                if k in override:
                    pg.__dict__[k] = override[k]
        # print(override)
        del override
    return pg.start()

def stop_download(name, url):
    url_status = context['PluginInfo'].url_status

    # Try to safely stop any download associated with the URL
    if url_status[url] == 1:
        logger.info(f"尝试停止下载 {name} - {url}")

        # Check if there's an ongoing download in the map
        download_proc = context["sync_downloader_map"].get(name)
        if download_proc:
            try:
                download_proc.terminate()  # Send termination signal to the FFmpeg process
                download_proc.wait()  # Wait for the process to terminate
                logger.info(f"Download process for {name} - {url} has been stopped.")
            except Exception as e:
                logger.error(f"Error while stopping the download: {e}")

def biliup_download(name, url, kwargs: dict):
    kwargs.pop('url')
    suffix = kwargs.get('format')
    if suffix:
        kwargs['suffix'] = suffix
    return download(name, url, **kwargs)