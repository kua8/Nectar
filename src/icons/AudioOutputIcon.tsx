import AudioOutputSvg from './sf/headphones.svg?react';

interface AudioOutputIconProps {
  size?: number;
  className?: string;
  style?: React.CSSProperties;
}

export function AudioOutputIcon({ size = 24, className, style }: AudioOutputIconProps) {
  return <AudioOutputSvg width={size} height={size} className={className} style={style} />;
}
