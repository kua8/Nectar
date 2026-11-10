import SpeakerSlashSvg from './sf/speaker.slash.fill.svg?react';
import SpeakerWave1Svg from './sf/speaker.wave.1.fill.svg?react';
import SpeakerWave2Svg from './sf/speaker.wave.2.fill.svg?react';
import SpeakerWave3Svg from './sf/speaker.wave.3.fill.svg?react';

interface VolumeLevelIconProps {
  level: number;
  size?: number;
  className?: string;
  style?: React.CSSProperties;
}

export function VolumeLevelIcon({ level, size = 12, className, style }: VolumeLevelIconProps) {
  const Icon = level <= 0 ? SpeakerSlashSvg : level < 0.34 ? SpeakerWave1Svg : level < 0.67 ? SpeakerWave2Svg : SpeakerWave3Svg;
  return <Icon width={size} height={size} className={className} style={style} />;
}
