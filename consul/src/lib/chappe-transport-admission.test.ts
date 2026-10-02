import { create, fromBinary, toBinary } from '@bufbuild/protobuf';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { GatewaySubscribeSchema, GatewaySubscriptionAdmissionSchema } from '@/gen/marengo/v1/marengo_pb';
import { connectHttpStream, connectWebTransport, type ChappeTelemetryHandlers } from '@/lib/chappe-transport';
import { setRuntimeCredential } from '@/lib/runtime-credentials';

vi.mock('@/lib/chappe-config', () => ({
  getChappeEndpoints: () => ({httpUrl:'http://127.0.0.1:1',webTransportUrl:'https://127.0.0.1:2/chappe'}),
  getChappeSubscribeTopics: () => ['robot/heartbeat','logs/structured'],
}));

const sessions: FixtureTransport[]=[];
class FixtureTransport {
  ready=Promise.resolve();
  closed=new Promise<void>(()=>{});
  controller!:ReadableStreamDefaultController<Uint8Array>;
  subscription:ReturnType<typeof create<typeof GatewaySubscribeSchema>>|undefined;
  isClosed=false;
  constructor() { sessions.push(this); }
  createBidirectionalStream() {
    let prefix=true;
    return Promise.resolve({
      readable:new ReadableStream<Uint8Array>({start:(controller)=>{this.controller=controller;}}),
      writable:new WritableStream<Uint8Array>({write:(value)=>{
        if (prefix) prefix=false;
        else this.subscription=fromBinary(GatewaySubscribeSchema,value);
      }}),
    });
  }
  close() { if (!this.isClosed) { this.isClosed=true; this.controller?.close(); } }
  admit(status:number) {
    const payload=toBinary(GatewaySubscriptionAdmissionSchema,create(GatewaySubscriptionAdmissionSchema,{status,topics:status===200?[...this.subscription!.topics,'gateway/runtime_connection']:[]}));
    const bytes=new Uint8Array(payload.length+4); new DataView(bytes.buffer).setUint32(0,payload.length,true); bytes.set(payload,4);
    this.controller.enqueue(bytes);
  }
}
function handlers():ChappeTelemetryHandlers {
  return {onRobotState:vi.fn(),onSafetyState:vi.fn(),onHeartbeat:vi.fn(),onConnected:vi.fn(),onDisconnected:vi.fn(),onError:vi.fn()};
}
beforeEach(()=>{
  setRuntimeCredential('operator',''); setRuntimeCredential('sensitiveRead',''); setRuntimeCredential('control','');
  sessions.length=0;
  vi.stubGlobal('WebTransport',FixtureTransport);
  vi.stubGlobal('fetch',vi.fn(async()=>new Response(JSON.stringify({algorithm:'sha-256',value:'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA='}),{status:200})));
});
afterEach(()=>{
  for(const session of sessions)session.close();
  setRuntimeCredential('operator',''); setRuntimeCredential('sensitiveRead',''); setRuntimeCredential('control','');
  vi.useRealTimers(); vi.unstubAllGlobals();
});

it('announces connection only after typed admission and subscribes publicly without a read credential',async()=>{
  const callbacks=handlers(); const pending=connectWebTransport(callbacks,()=>false);
  await vi.waitFor(()=>expect(sessions[0]?.subscription).toBeDefined());
  expect(sessions[0].subscription!.topics).toEqual(['robot/heartbeat']);
  expect(sessions[0].subscription!.runtimeCredential).toBe('');
  expect(callbacks.onConnected).not.toHaveBeenCalled();
  sessions[0].admit(200);
  const stop=await pending;
  expect(callbacks.onConnected).toHaveBeenCalledOnce(); stop!();
});

it('supplies the runtime read credential for sensitive topics and keeps denied sessions disconnected',async()=>{
  setRuntimeCredential('sensitiveRead','isolated-read-fixture');
  const callbacks=handlers(); const pending=connectWebTransport(callbacks,()=>false);
  const rejected=expect(pending).rejects.toThrow('refused: 403');
  await vi.waitFor(()=>expect(sessions[0]?.subscription).toBeDefined());
  expect(sessions[0].subscription!.topics).toContain('logs/structured');
  expect(sessions[0].subscription!.runtimeCredential).toBe('isolated-read-fixture');
  sessions[0].admit(403); await rejected;
  expect(callbacks.onConnected).not.toHaveBeenCalled(); expect(sessions[0].isClosed).toBe(true);
});

it('bounds a missing admission response and closes the owned transport',async()=>{
  vi.useFakeTimers(); const callbacks=handlers();
  const pending=connectWebTransport(callbacks,()=>false);
  const rejected=expect(pending).rejects.toThrow('subscription timed out');
  await vi.waitFor(()=>expect(sessions[0]?.subscription).toBeDefined());
  await vi.advanceTimersByTimeAsync(5000); await rejected;
  expect(callbacks.onConnected).not.toHaveBeenCalled(); expect(sessions[0].isClosed).toBe(true);
});

it('HTTP fallback filters public topics and carries read credentials only in headers',async()=>{
  const controllers:ReadableStreamDefaultController<Uint8Array>[]=[];
  const fetchMock=vi.fn(async(_url:string,_init?:RequestInit)=>new Response(new ReadableStream<Uint8Array>({start:(controller)=>controllers.push(controller)}),{status:200}));
  vi.stubGlobal('fetch',fetchMock);
  const stop=await connectHttpStream(handlers(),()=>false);
  expect(fetchMock.mock.calls[0][0]).not.toContain('logs%2Fstructured');
  expect(new Headers(fetchMock.mock.calls[0][1]!.headers).has('authorization')).toBe(false);
  stop!();
  setRuntimeCredential('sensitiveRead','isolated-http-read-fixture');
  const stopRead=await connectHttpStream(handlers(),()=>false);
  const [url,init]=fetchMock.mock.calls[1];
  expect(url).toContain('logs%2Fstructured'); expect(url).not.toContain('fixture');
  expect(new Headers(init!.headers).get('authorization')).toBe('Bearer isolated-http-read-fixture');
  stopRead!();
});
