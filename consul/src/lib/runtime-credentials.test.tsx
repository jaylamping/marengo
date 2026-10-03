import { create } from '@bufbuild/protobuf';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import { RuntimeCredentialField } from '@/components/dashboard/site-header/runtime-credential-field';
import { MitCommandBatchSchema, OperatorCommandSchema } from '@/gen/marengo/v1/marengo_pb';
import { autoLearnConfig, postAutoLearn } from '@/lib/auto-learn-api';
import { patchConfig, restartMarengoPi } from '@/lib/config-api';
import { postEnableCommand, postTestingMitCommandBatch, postSetZeroCommand, postActuatorCommand, putCommissioningScope, deleteCommissioningScope } from '@/lib/gateway-api';
import { fetchUrdfArchiveList } from '@/lib/hardware-api';
import { fetchRecentLogs } from '@/lib/log-api';
import { startSelfDeploy } from '@/lib/version-api';
import { gatewayAuthHeaders, gatewayCredential, runtimeCredential, setRuntimeCredential, type CredentialPurpose } from '@/lib/runtime-credentials';

vi.mock('@/lib/chappe-config', () => ({ getChappeEndpoints: () => ({ httpUrl: 'http://127.0.0.1:1', webTransportUrl: 'https://127.0.0.1:2/chappe' }) }));
const purposes: CredentialPurpose[] = ['operator','control','calibration','configuration','management','sensitiveRead','autoLearn'];
const fetchMock = vi.fn();
beforeEach(() => {
  for (const purpose of purposes) setRuntimeCredential(purpose,'');
  fetchMock.mockReset().mockImplementation(async () => new Response(JSON.stringify({ ok:true,message:'isolated fetch fixture',persist_status:'durable',entries:[] }),{status:200}));
  vi.stubGlobal('fetch',fetchMock);
});
afterEach(() => {
  cleanup();
  for (const purpose of purposes) setRuntimeCredential(purpose,'');
  vi.unstubAllGlobals(); vi.unstubAllEnvs();
});

it('retired build credentials do not provide runtime authorization', () => {
  vi.stubEnv('VITE_MARENGO_LOG_TOKEN','isolated-retired-build-fixture');
  vi.stubEnv('VITE_AUTO_LEARN_TOKEN','isolated-retired-auto-build-fixture');
  expect(gatewayAuthHeaders('control')).toEqual({});
  expect(autoLearnConfig().token).toBeNull();
});

it('actual command/configuration/log/management clients select the runtime capability credential', async () => {
  for (const purpose of purposes) setRuntimeCredential(purpose,`isolated-${purpose}-fixture`);
  const cases: Array<[() => Promise<unknown>,string]> = [
    [() => postEnableCommand(false),'control'],
    [() => postTestingMitCommandBatch(create(MitCommandBatchSchema)),'control'],
    [() => postActuatorCommand(create(OperatorCommandSchema)),'control'],
    [() => postSetZeroCommand('right_shoulder_pitch',{signTestPassed:true}),'calibration'],
    [() => patchConfig({joint:'right_shoulder_pitch', expected_revision:'fixture-revision'}),'configuration'],
    [() => putCommissioningScope({joints:[],confirm_widen:false}),'configuration'],
    [() => deleteCommissioningScope(),'configuration'],
    [() => fetchUrdfArchiveList(),'configuration'],
    [() => fetchRecentLogs(),'sensitiveRead'],
    [() => restartMarengoPi(),'management'],
    [() => startSelfDeploy(),'management'],
  ];
  for (const [request,purpose] of cases) {
    await request();
    const [url,init]=fetchMock.mock.calls.at(-1)!;
    expect(new Headers(init.headers).get('authorization')).toBe(`Bearer isolated-${purpose}-fixture`);
    expect(url).not.toContain('fixture');
    expect(new Headers(init.headers).has('x-marengo-log-token')).toBe(false);
  }
});

it('runtime entry updates the shared client state and clearing it removes authorization without storage', () => {
  const localWrite=vi.spyOn(Storage.prototype,'setItem');
  render(<RuntimeCredentialField purpose="operator" label="Gateway credential" />);
  const field=screen.getByLabelText('Gateway credential');
  expect(field.getAttribute('type')).toBe('password');
  fireEvent.change(field,{target:{value:'isolated-ui-entry-fixture'}});
  expect(gatewayCredential('configuration')).toBe('isolated-ui-entry-fixture');
  expect(gatewayAuthHeaders('control').Authorization).toBe('Bearer isolated-ui-entry-fixture');
  fireEvent.change(field,{target:{value:''}});
  expect(gatewayAuthHeaders('control')).toEqual({});
  expect(localWrite).not.toHaveBeenCalled();
  localWrite.mockRestore();
});

it('scope credentials do not silently populate another capability and operator fallback is explicit', () => {
  setRuntimeCredential('control','isolated-control-fixture');
  expect(gatewayCredential('sensitiveRead')).toBe('');
  setRuntimeCredential('operator','isolated-operator-fixture');
  expect(gatewayCredential('control')).toBe('isolated-control-fixture');
  expect(gatewayCredential('management')).toBe('isolated-operator-fixture');
  expect(() => setRuntimeCredential('operator','\u0001invalid')).toThrow();
  expect(() => setRuntimeCredential('operator','é'.repeat(2049))).toThrow();
  expect(runtimeCredential('operator')).toBe('isolated-operator-fixture');
});

it('Auto Learn uses its separate runtime entry and refuses a missing credential before fetch', async () => {
  vi.stubEnv('VITE_AUTO_LEARN_URL','http://127.0.0.1:1');
  const request = {} as Parameters<typeof postAutoLearn>[0];
  expect((await postAutoLearn(request)).ok).toBe(false);
  expect(fetchMock).not.toHaveBeenCalled();
  setRuntimeCredential('autoLearn','isolated-auto-runtime-fixture');
  expect((await postAutoLearn(request)).ok).toBe(true);
  expect(fetchMock.mock.calls[0][1].headers.authorization).toBe('Bearer isolated-auto-runtime-fixture');
});
