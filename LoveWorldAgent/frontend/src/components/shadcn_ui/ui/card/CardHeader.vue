<script setup>
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <div :class="cn('flex flex-col gap-1 px-0.5', props.class)">
        <slot />
    </div>
</template>
