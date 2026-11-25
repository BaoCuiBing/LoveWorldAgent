<script setup>
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    as: { type: [String, Object], default: 'h3' },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <component :is="props.as" :class="cn('text-sm font-semibold leading-none tracking-tight text-card-foreground', props.class)">
        <slot />
    </component>
</template>
