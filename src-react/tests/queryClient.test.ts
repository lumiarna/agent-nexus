import test from "node:test";
import assert from "node:assert/strict";

import { APP_QUERY_STALE_TIME, createAppQueryClient } from "../src/lib/query/client.js";

test("app query client keeps cached data fresh across tab remounts", () => {
  const queryClient = createAppQueryClient();

  try {
    const queries = queryClient.getDefaultOptions().queries;
    assert.equal(queries?.staleTime, APP_QUERY_STALE_TIME);
    assert.equal(queries?.staleTime, Infinity);
    assert.equal(queries?.refetchOnWindowFocus, false);
  } finally {
    queryClient.clear();
  }
});
