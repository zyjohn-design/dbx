<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Loader2, Pencil, Plus, Server, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { loadRemoteWorkspaces, normalizeRemoteWorkspaceUrl, openRemoteWorkspace, remoteWorkspaceErrorCode, removeRemoteWorkspace, saveRemoteWorkspace, testRemoteWorkspace, type RemoteWorkspace } from "@/lib/app/remoteWorkspaces";
import { uuid } from "@/lib/common/utils";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ "update:open": [value: boolean] }>();
const { t } = useI18n();
const workspaces = ref<RemoteWorkspace[]>([]);
const draft = ref<RemoteWorkspace | null>(null);
const pendingRemoval = ref<RemoteWorkspace | null>(null);
const busy = ref(false);
const error = ref("");
const status = ref("");
const formValid = computed(() => !!draft.value?.name.trim() && !!draft.value?.url.trim());

function reportError(value: unknown) {
  error.value = t(`remoteWorkspace.errors.${remoteWorkspaceErrorCode(value)}`);
}

async function run(action: () => Promise<void>) {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  status.value = "";
  try {
    await action();
  } catch (value) {
    reportError(value);
  } finally {
    busy.value = false;
  }
}

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    draft.value = null;
    pendingRemoval.value = null;
    void run(async () => {
      workspaces.value = await loadRemoteWorkspaces();
    });
  },
  { immediate: true },
);

function edit(workspace?: RemoteWorkspace) {
  draft.value = workspace ? { ...workspace } : { id: uuid(), name: "", url: "" };
  pendingRemoval.value = null;
  error.value = "";
  status.value = "";
}

function updateUrl(value: string | number) {
  if (draft.value) draft.value.url = String(value);
  status.value = "";
}

function test() {
  const url = draft.value?.url;
  if (!url) return;
  void run(async () => {
    await testRemoteWorkspace(url);
    status.value = t("remoteWorkspace.testSuccess");
  });
}

function save() {
  if (!draft.value || !formValid.value) return;
  const workspace = { ...draft.value };
  void run(async () => {
    workspace.url = normalizeRemoteWorkspaceUrl(workspace.url);
    await testRemoteWorkspace(workspace.url);
    workspaces.value = await saveRemoteWorkspace(workspace);
    draft.value = null;
  });
}

function connect(workspace: RemoteWorkspace) {
  void run(async () => {
    await openRemoteWorkspace(workspace.id);
  });
}

function remove() {
  const workspace = pendingRemoval.value;
  if (!workspace) return;
  void run(async () => {
    workspaces.value = await removeRemoteWorkspace(workspace.id);
    pendingRemoval.value = null;
  });
}

function close(open: boolean) {
  if (!busy.value) emit("update:open", open);
}
</script>

<template>
  <Dialog :open="open" @update:open="close">
    <DialogContent class="sm:max-w-xl" @escape-key-down="busy && $event.preventDefault()" @interact-outside="busy && $event.preventDefault()">
      <DialogHeader>
        <DialogTitle>{{ t("remoteWorkspace.title") }}</DialogTitle>
        <DialogDescription>{{ t("remoteWorkspace.description") }}</DialogDescription>
      </DialogHeader>

      <p v-if="error" role="alert" class="text-sm text-destructive">{{ error }}</p>
      <p v-if="status" role="status" class="text-sm text-emerald-600">{{ status }}</p>

      <form v-if="draft" class="space-y-4" @submit.prevent="save">
        <label class="block space-y-1.5 text-sm">
          <span>{{ t("remoteWorkspace.name") }}</span>
          <Input v-model="draft.name" :disabled="busy" :maxlength="80" required :placeholder="t('remoteWorkspace.namePlaceholder')" />
        </label>
        <label class="block space-y-1.5 text-sm">
          <span>{{ t("remoteWorkspace.url") }}</span>
          <Input :model-value="draft?.url ?? ''" :disabled="busy" type="url" required placeholder="https://dbx.example.com/dbx/" @update:model-value="updateUrl" />
        </label>
        <p class="text-xs text-muted-foreground">{{ t("remoteWorkspace.loginHint") }}</p>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            :disabled="busy"
            @click="
              draft = null;
              error = '';
              status = '';
            "
            >{{ t("common.cancel") }}</Button
          >
          <Button type="button" variant="outline" :disabled="busy || !draft?.url.trim()" @click="test">{{ t("remoteWorkspace.test") }}</Button>
          <Button type="submit" :disabled="busy || !formValid"><Loader2 v-if="busy" class="mr-1.5 h-4 w-4 animate-spin" />{{ t("common.save") }}</Button>
        </DialogFooter>
      </form>

      <div v-else-if="pendingRemoval" class="space-y-4">
        <p class="text-sm">{{ t("remoteWorkspace.removeConfirm", { name: pendingRemoval.name }) }}</p>
        <DialogFooter>
          <Button variant="outline" :disabled="busy" @click="pendingRemoval = null">{{ t("common.cancel") }}</Button>
          <Button variant="destructive" :disabled="busy" @click="remove">{{ t("common.remove") }}</Button>
        </DialogFooter>
      </div>

      <template v-else>
        <div class="max-h-80 space-y-2 overflow-auto">
          <p v-if="!workspaces.length && !busy" class="py-8 text-center text-sm text-muted-foreground">{{ t("remoteWorkspace.empty") }}</p>
          <div v-for="workspace in workspaces" :key="workspace.id" class="flex items-center gap-2 rounded-md border p-3">
            <Server class="h-4 w-4 shrink-0 text-muted-foreground" />
            <div class="min-w-0 flex-1">
              <p class="truncate text-sm font-medium">{{ workspace.name }}</p>
              <p class="truncate text-xs text-muted-foreground" :title="workspace.url">{{ workspace.url }}</p>
            </div>
            <Button variant="ghost" size="icon" :aria-label="t('remoteWorkspace.edit')" :disabled="busy" @click="edit(workspace)"><Pencil class="h-4 w-4" /></Button>
            <Button
              variant="ghost"
              size="icon"
              :aria-label="t('common.remove')"
              :disabled="busy"
              @click="
                pendingRemoval = workspace;
                error = '';
              "
              ><Trash2 class="h-4 w-4"
            /></Button>
            <Button size="sm" :disabled="busy" @click="connect(workspace)">{{ t("remoteWorkspace.connect") }}</Button>
          </div>
        </div>
        <DialogFooter>
          <Loader2 v-if="busy" role="status" :aria-label="t('common.loading')" class="mr-auto h-4 w-4 animate-spin" />
          <Button :disabled="busy" @click="edit()"><Plus class="mr-1.5 h-4 w-4" />{{ t("remoteWorkspace.add") }}</Button>
        </DialogFooter>
      </template>
    </DialogContent>
  </Dialog>
</template>
