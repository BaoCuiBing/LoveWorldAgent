<script setup>
import { SelectValue } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    placeholder: { type: String, default: '' },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <SelectValue :placeholder="props.placeholder" :class="cn('truncate', props.class)" />
</template>
