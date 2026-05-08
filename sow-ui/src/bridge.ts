export interface ToWorkerMessage {
  type: 'attack' | 'build' | 'fleet' | 'cancel' | 'input';
  payload: any;
}

export interface PlayerSummary {
  id: number;
  name: string;
  troops: number;
  gold: number;
  tileCount: number;
  alive: boolean;
  color: [number, number, number];
}

export interface GameEvent {
  type: string;
  [key: string]: any;
}

export interface FromWorkerMessage {
  type: 'state_update' | 'event';
  payload: {
    tick: number;
    myPlayer?: PlayerSummary;
    players: PlayerSummary[];
    events: GameEvent[];
  };
}
