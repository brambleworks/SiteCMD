import { request } from "node:http";

export function bridgeRequest(socketPath, route, body) {
  return new Promise((resolve, reject) => {
    const requestBody = JSON.stringify(body);
    const outgoing = request(
      {
        socketPath,
        path: route,
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Content-Length": Buffer.byteLength(requestBody),
        },
      },
      (response) => {
        const chunks = [];
        let size = 0;
        response.on("data", (chunk) => {
          size += chunk.length;
          if (size > 8 * 1024 * 1024) outgoing.destroy(new Error("Benchmark response too large"));
          else chunks.push(chunk);
        });
        response.on("end", () => {
          try {
            const value = JSON.parse(Buffer.concat(chunks));
            if (response.statusCode !== 200) reject(new Error(value.error));
            else resolve(value);
          } catch (error) {
            reject(error);
          }
        });
      },
    );
    outgoing.on("error", reject);
    outgoing.setTimeout(150000, () => outgoing.destroy(new Error("Benchmark bridge timed out")));
    outgoing.end(requestBody);
  });
}
