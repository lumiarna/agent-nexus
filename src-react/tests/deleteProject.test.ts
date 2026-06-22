import test from "node:test";
import assert from "node:assert/strict";

import { QueryClient, QueryObserver } from "@tanstack/react-query";

import {
  createDeleteProjectMutationOptions,
  projectKeys,
} from "../src/lib/query/projectDeletion.js";

test("deleting a project refetches the active projects query", async () => {
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
    queryKey: projectKeys.all,
    queryFn,
    staleTime: Infinity,
  });

  const observer = new QueryObserver(queryClient, {
    queryKey: projectKeys.all,
    queryFn,
    staleTime: Infinity,
  });
  const unsubscribe = observer.subscribe(() => undefined);

  try {
    const options = createDeleteProjectMutationOptions(queryClient, async () => undefined);
    await options.onSuccess();
    assert.equal(fetchCount, 2);
  } finally {
    unsubscribe();
    queryClient.clear();
  }
});
