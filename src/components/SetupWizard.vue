<script setup lang="ts">
import { ref } from "vue";
import {
  busy,
  chooseRuntimeFolder,
  dismissSetupWizard,
  installRuntime,
  runtimePath,
  updateProgress,
  useDefaultRuntimeFolder,
} from "../store";

const step = ref<1 | 2 | 3>(1);
</script>

<template>
  <div class="setup-wizard">
    <div class="setup-card">
      <div class="setup-steps" aria-hidden="true">
        <span :class="{ active: step >= 1 }">1</span>
        <span :class="{ active: step >= 2 }">2</span>
        <span :class="{ active: step >= 3 }">3</span>
      </div>

      <div v-show="step === 1">
        <h2>Добро пожаловать в zapret-ui</h2>
        <p class="setup-text">
          Приложение управляет обходом DPI для Discord и YouTube. Для работы нужен flowseal-zapret
          (zapret-discord-youtube от Flowseal): около 10 МБ, скачивается с GitHub и распаковывается
          локально.
        </p>
        <ul class="setup-list">
          <li>Windows 11 x64 (build 22000 или новее)</li>
          <li>Права администратора при запуске — нужны для WinDivert</li>
          <li>WebView2 в Windows 11 уже установлен</li>
        </ul>
        <div class="setup-actions">
          <button class="btn small" @click="dismissSetupWizard">Настроить позже</button>
          <button class="btn primary" @click="step = 2">Продолжить</button>
        </div>
      </div>

      <div v-show="step === 2">
        <h2>Где хранить flowseal-zapret?</h2>
        <p class="setup-text">
          В выбранной папке будут стратегии <code>.bat</code>, списки и <code>bin/winws.exe</code>.
          Можно оставить путь по умолчанию или выбрать другую папку — например, рядом с портативным
          exe.
        </p>

        <div class="setup-path" :title="runtimePath">{{ runtimePath || "Путь по умолчанию" }}</div>

        <div class="setup-actions setup-actions-wrap">
          <button class="btn small" @click="step = 1">Назад</button>
          <button class="btn small" @click="useDefaultRuntimeFolder">По умолчанию</button>
          <button class="btn small" @click="chooseRuntimeFolder">Изменить</button>
          <button class="btn primary" @click="step = 3">Продолжить</button>
        </div>
      </div>

      <div v-show="step === 3">
        <h2>Скачать flowseal-zapret</h2>
        <p class="setup-text">Файл будет скачан с GitHub, проверен по SHA-256 и распакован в:</p>
        <div class="setup-path" :title="runtimePath">{{ runtimePath || "Путь по умолчанию" }}</div>

        <div v-if="updateProgress" class="muted setup-progress">{{ updateProgress }}</div>

        <div class="setup-actions setup-actions-wrap">
          <button class="btn small" :disabled="busy" @click="step = 2">Назад</button>
          <button class="btn primary" :disabled="busy" @click="installRuntime">
            {{ busy ? "Установка…" : "Скачать и установить" }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
