<script setup>
import { AlertTriangle, CheckCircle2, Info, X, XCircle } from 'lucide-vue-next'
import { toast, toasts } from './useToast'
import { cn } from './utils'
const icons = {
    success: CheckCircle2,
    error: XCircle,
    warning: AlertTriangle,
    info: Info,
}
const toneClass = {
    success: 'text-emerald-500',
    error: 'text-destructive',
    warning: 'text-amber-500',
    info: 'text-primary',
}
</script>

<template>
    <div class="pointer-events-none fixed inset-x-0 top-3 z-[100] flex flex-col items-center gap-2">
        <div
            v-for="item in toasts"
            :key="item.id"
            :class="
                cn(
                    'pointer-events-auto flex w-fit min-w-48 max-w-md items-start gap-2 rounded-md border border-border bg-popover px-3 py-2 text-xs text-popover-foreground shadow-lg',
                    'animate-in fade-in-0 slide-in-from-top-2',
                )
            "
        >
            <component :is="icons[item.type] || Info" :class="cn('mt-px size-3.5 shrink-0', toneClass[item.type] || 'text-primary')" />
            <div class="flex-1 leading-5">
                <div v-if="item.title" class="font-medium">{{ item.title }}</div>
                <div>{{ item.message }}</div>
            </div>
            <button class="mt-px shrink-0 text-muted-foreground transition-colors hover:text-foreground" @click="toast.dismiss(item.id)">
                <X class="size-3.5" />
            </button>
        </div>
    </div>
</template>
