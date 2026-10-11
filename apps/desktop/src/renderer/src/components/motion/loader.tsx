'use client'
// Adapted from beui.dev/components/motion/loader; only the spinner is installed.

import { motion, useReducedMotion } from 'motion/react'
import { EASE_IN_OUT } from '@/lib/ease'

export function Loader({ size = 14 }: { size?: number }): React.JSX.Element {
  const reduce = useReducedMotion()
  const stroke = Math.max(2, size * 0.09)
  const radius = (size - stroke) / 2
  return (
    <motion.svg
      aria-hidden="true"
      width={size}
      height={size}
      viewBox={`0 0 ${String(size)} ${String(size)}`}
      animate={reduce ? { opacity: [1, 0.4, 1] } : { rotate: 360 }}
      transition={
        reduce
          ? { duration: 1.4, ease: EASE_IN_OUT, repeat: Infinity }
          : { duration: 1, ease: 'linear', repeat: Infinity }
      }
    >
      <circle
        cx={size / 2}
        cy={size / 2}
        r={radius}
        fill="none"
        stroke="currentColor"
        strokeOpacity={0.2}
        strokeWidth={stroke}
      />
      <path
        d={`M ${String(size / 2)} ${String(size / 2 - radius)} A ${String(radius)} ${String(radius)} 0 0 1 ${String(size / 2 + radius)} ${String(size / 2)}`}
        fill="none"
        stroke="currentColor"
        strokeWidth={stroke}
        strokeLinecap="round"
      />
    </motion.svg>
  )
}
