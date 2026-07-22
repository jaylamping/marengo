/** sessionStorage key for operator-pasted Auto Learn token (Pi www). */
export const SESSION_KEY = 'marengo.autoLearnOperatorToken';

/**
 * Prefer sessionStorage (browser paste), else Vite `VITE_AUTO_LEARN_OPERATOR_TOKEN`.
 * Static `import.meta.env` access only — do not use dynamic env keys.
 */
export function getAutoLearnOperatorToken(): string | null {
  if (typeof window !== 'undefined') {
    const fromSession = window.sessionStorage.getItem(SESSION_KEY);
    const trimmedSession = fromSession?.trim();
    if (trimmedSession) {
      return trimmedSession;
    }
  }
  // Static import.meta.env.*. Dynamic import.meta.env[key] makes Vite inline
  // the entire env object into production dist.
  const fromEnv = (
    import.meta.env.VITE_AUTO_LEARN_OPERATOR_TOKEN as string | undefined
  )?.trim();
  return fromEnv || null;
}

/** Set or clear the session operator token. No-op without `window`. */
export function setAutoLearnOperatorToken(token: string | null): void {
  if (typeof window === 'undefined') {
    return;
  }
  if (token == null || token.trim() === '') {
    window.sessionStorage.removeItem(SESSION_KEY);
    return;
  }
  window.sessionStorage.setItem(SESSION_KEY, token.trim());
}
