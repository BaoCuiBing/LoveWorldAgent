<script setup>
import { RadioGroupIndicator, RadioGroupItem } from 'reka-ui'
import { Circle } from 'lucide-vue-next'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    value: { type: String, required: true },
    disabled: Boolean,
    id: { type: String, default: undefined },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <RadioGroupItem
        :id="props.id"
        :value="props.value"
        :disabled="props.disabled"
        :class="cn('flex size-3.5 shrink-0 items-center justify-center rounded-full border border-input text-primary shadow-sm outline-none transition-colors focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:border-primary', props.class)"
    >
        <RadioGroupIndicator class="flex items-center justify-center">
            <Circle class="size-2 fill-primary text-primary" />
        </RadioGroupIndicator>
    </RadioGroupItem>
</template>
