<script setup>
import { inject, computed } from 'vue'
import { Search } from 'lucide-vue-next'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    placeholder: { type: String, default: '搜索…' },
    class: { type: [String, Array, Object], default: '' },
})
const command = inject('command', null)
const model = computed({
    get: () => (command ? command.query.value : ''),
    set: (value) => {
        if (command) command.query.value = value
    },
})
</script>

<template>
    <div :class="cn('flex h-8 items-center gap-2 border-b border-border px-2.5', props.class)">
        <Search class="size-3.5 shrink-0 text-muted-foreground" />
        <input
            v-model="model"
            :placeholder="props.placeholder"
            class="h-full w-full bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
        />
    </div>
</template>
