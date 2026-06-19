import test from "node:test";
import assert from "node:assert/strict";

import { QueryClient, QueryObserver } from "@tanstack/react-query";

import {
  createDeleteProjectSymlinkMutationOptions,
  projectSymlinkKeys,
} from "../src/lib/query/projectSymlinkInventory.js";

test("deleting a project symlink refetches the active project symlink inventory query", async () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
    },
  });
  let fetchCount = 0;
  const queryFn = async () => {
    fetchCount += 1;
    return [];
  };

  await queryClient.fetchQuery({
    queryKey: projectSymlinkKeys.inventory,
    queryFn,
    staleTime: Infinity,
  });

  const observer = new QueryObserver(queryClient, {
    queryKey: projectSymlinkKeys.inventory,
    queryFn,
    staleTime: Infinity,
  });
  const unsubscribe = observer.subscribe(() => undefined);

  try {
    const mutation = createDeleteProjectSymlinkMutationOptions(queryClient, async () => undefined);
    await mutation.onSuccess();
    assert.equal(fetchCount, 2);
  } finally {
    unsubscribe();
    queryClient.clear();
  }
});
