<script setup>
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    as: { type: [String, Object], default: 'div' },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <component :is="props.as" :class="cn('flex flex-col gap-2 rounded-lg border border-border bg-card p-3 text-card-foreground shadow-sm', props.class)">
        <slot />
    </component>
</template>
