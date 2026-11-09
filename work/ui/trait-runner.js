function createTraitRunner(traitData) {
  const eligible = new Set(traitData.traits.filter(trait => trait.random).map(trait => trait.key));

  // The same entry point runs inside the offline worker and in browsers that
  // do not allow a worker on a file URL.
  function planRequest(request) {
    const inherited = TraitSolver.solve({...request,allowRandom:false});
    if (!request.allowRandom || inherited.status === 'ok' || inherited.status === 'owned' ||
        inherited.status === 'invalid' || !request.randomEligible.length) return inherited;
    return TraitSolver.solve(request);
  }
  const run = createOfflineRunner(planRequest,['trait-solver-code']);
  return input => run({...input,randomEligible:input.traits.filter(key => eligible.has(key))});
}

function createOfflineRunner(planRequest, scriptIds) {
  let active=null;
  const workerCode = scriptIds.map(id => document.getElementById(id).textContent).join('\n;\n') +
    '\n;self.onmessage = function(event) { try {' +
    'const result = (' + planRequest.toString() + ')(event.data);' +
    'self.postMessage({result});' +
    '} catch (error) {self.postMessage({error:String(error.message || error)});}};';

  return function run(request) {
    if (active) active();
    return new Promise((resolve,reject) => {
      let worker, url, fallback;
      function cleanup() {
        if (worker) { worker.onmessage=null; worker.onerror=null; worker.terminate(); }
        if (url) URL.revokeObjectURL(url);
        clearTimeout(fallback);
        active=null;
      }
      active = () => {
        cleanup();
        reject(new DOMException('已开始新的计算。','AbortError'));
      };
      function useMainThread() {
        fallback=setTimeout(() => {
          try { const result=planRequest(request); cleanup(); resolve(result); }
          catch (error) { cleanup(); reject(error); }
        },0);
      }
      try {
        url = URL.createObjectURL(new Blob([workerCode],{type:'text/javascript'}));
        worker = new Worker(url);
      } catch { useMainThread(); return; }

      worker.onmessage = ({data}) => {
        cleanup();
        if (data.error) reject(new Error(data.error));
        else resolve(data.result);
      };
      worker.onerror = event => {
        event.preventDefault();
        worker.terminate();
        useMainThread();
      };
      try { worker.postMessage(request); }
      catch (error) { cleanup(); reject(error); }
    });
  };
}
