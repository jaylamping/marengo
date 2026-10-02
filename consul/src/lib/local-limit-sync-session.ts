/** Local mirror credentials stay in this tab's memory until reload. */
let credential = '';

export function localLimitSyncCredential(): string {
  return credential;
}

export function setLocalLimitSyncCredential(value: string): void {
  credential = value.trim();
}
