// Run only on the generated fixture, outside measured intervals.
(async () => {
  if (document.title !== "Karere generated media probe" || location.hostname !== "127.0.0.1") {
    throw new Error("graphics check requires the generated local fixture");
  }
  const result = {};
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 1;
  const gl = canvas.getContext("webgl2");
  if (gl) {
    gl.clearColor(.25, .5, .75, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    const pixels = new Uint8Array(4);
    gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
    const debug = gl.getExtension("WEBGL_debug_renderer_info");
    result.webgl2 = {
      available: true, pixels: [...pixels],
      outputCorrect: [...pixels].every((value, i) => Math.abs(value - [64, 128, 191, 255][i]) <= 1),
      renderer: debug ? gl.getParameter(debug.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
      vendor: debug ? gl.getParameter(debug.UNMASKED_VENDOR_WEBGL) : gl.getParameter(gl.VENDOR),
    };
    gl.getExtension("WEBGL_lose_context")?.loseContext();
  } else {
    result.webgl2 = {available: false};
  }
  try {
    const adapter = await navigator.gpu?.requestAdapter({powerPreference: "high-performance"});
    if (!adapter) {
      result.webgpu = {available: false, reason: "no adapter"};
    } else {
      const info = adapter.info;
      result.webgpu = {available: true, vendor: info.vendor, architecture: info.architecture,
        device: info.device, description: info.description,
        fallbackAdapter: info.isFallbackAdapter ?? adapter.isFallbackAdapter ?? null};
      const device = await adapter.requestDevice();
      try {
        const storage = device.createBuffer({size: 4, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC});
        const output = device.createBuffer({size: 4, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST});
        const pipeline = device.createComputePipeline({layout: "auto", compute: {
          module: device.createShaderModule({code: `
            @group(0) @binding(0) var<storage, read_write> output: u32;
            @compute @workgroup_size(1) fn main() { output = 305419896u; }
          `}), entryPoint: "main",
        }});
        const commands = device.createCommandEncoder();
        const pass = commands.beginComputePass();
        pass.setPipeline(pipeline);
        pass.setBindGroup(0, device.createBindGroup({layout: pipeline.getBindGroupLayout(0),
          entries: [{binding: 0, resource: {buffer: storage}}]}));
        pass.dispatchWorkgroups(1); pass.end();
        commands.copyBufferToBuffer(storage, 0, output, 0, 4);
        device.queue.submit([commands.finish()]);
        await output.mapAsync(GPUMapMode.READ);
        result.webgpu.outputCorrect = new Uint32Array(output.getMappedRange())[0] === 305419896;
        output.unmap(); storage.destroy(); output.destroy();
      } finally {
        device.destroy();
      }
    }
  } catch (error) {
    result.webgpu = {...result.webgpu, outputCorrect: false, error: String(error)};
  }
  return result;
})()
