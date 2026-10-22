// Authority comes from the browser's live document context, never a window,
// profile or URL supplied in a UI request.
export function isGameHubSender(page) {
  const sender = page?.sender;
  if (page?.name !== 'omniplay-ui' || sender?.id !== chrome.runtime.id ||
      sender.frameId !== 0 || sender.tab?.incognito ||
      typeof sender.documentId !== 'string') return false;
  return isGameHubUrl(sender.url);
}

function isGameHubUrl(value) {
  try {
    const url = new URL(value);
    return url.protocol === 'chrome-extension:' && url.hostname === chrome.runtime.id &&
      url.pathname === '/index.html' && !url.search && !url.username && !url.password;
  } catch { return false; }
}

export async function gameHubContext(page, check = () => {}) {
  if (!isGameHubSender(page)) throw Object.assign(new Error('permission_denied'), {code:'permission_denied'});
  const documentId = page.sender.documentId;
  const contexts = await chrome.runtime.getContexts({documentIds:[documentId], contextTypes:['TAB']});
  check();
  const current = contexts.find(item => item.documentId === documentId &&
    item.contextType === 'TAB' && item.frameId === 0 && !item.incognito &&
    Number.isSafeInteger(item.windowId) && item.windowId >= 0 &&
    Number.isSafeInteger(item.tabId) && item.tabId >= 0 && isGameHubUrl(item.documentUrl));
  if (!current) throw Object.assign(new Error('permission_denied'), {code:'permission_denied'});
  return current;
}
